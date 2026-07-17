    fn verify_state_response_with_mempool_counts(pending_count: u64, sender_count: u64) -> RpcResponse {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        success_response(
            "state-atomic-senders",
            &json!({
                "schema": STATE_VERIFICATION_SCHEMA,
                "verified": true,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "block_log": {
                    "verified": true,
                    "block_count": 1,
                    "tip_hash": hex96,
                    "state_root": hex96
                },
                "governance": {
                    "verified": true,
                    "active_validator_count": 1,
                    "crypto_policy_version": 1,
                    "bridge_witness_epoch": 1,
                    "amendment_count": 0,
                    "latest_amendment_id": ""
                },
                "bridge": {
                    "verified": true,
                    "domain_count": 0,
                    "transfer_count": 0,
                    "attestation_count": 0,
                    "replay_cache_count": 0,
                    "inbound_used": 0,
                    "outbound_used": 0,
                    "latest_transfer_id": ""
                },
                "shielded": {
                    "verified": true,
                    "note_count": 0,
                    "nullifier_count": 0,
                    "turnstile_event_count": 0,
                    "tree_root": hex96,
                    "bootstrap_deposit_total": 0,
                    "migration_total": 0,
                    "orchard_deposit_total": 0,
                    "spent_note_count": 0,
                    "live_note_count": 0,
                    "latest_turnstile_event_id": ""
                },
                "mempool": {
                    "verified": true,
                    "pending_count": pending_count,
                    "sender_count": sender_count,
                    "total_amount": 0,
                    "total_fee": 0,
                    "latest_tx_id": if pending_count == 0 { "" } else { hex96 }
                }
            }),
            vec![],
        )
        .expect("verify-state response")
    }

    #[test]
    fn verify_state_accepts_two_atomic_swap_senders_for_one_pending_transaction() {
        let response = verify_state_response_with_mempool_counts(1, 2);
        validate_health_response(&response, RpcResponseKind::VerifyState)
            .expect("atomic-shaped mempool counts should validate");
    }

    #[test]
    fn verify_state_rejects_more_than_two_senders_per_pending_transaction() {
        let response = verify_state_response_with_mempool_counts(1, 3);
        assert!(matches!(
            validate_health_response(&response, RpcResponseKind::VerifyState),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "mempool.sender_count"
        ));
    }

    #[test]
    fn verify_state_sender_limit_is_overflow_safe() {
        let response = verify_state_response_with_mempool_counts(u64::MAX, u64::MAX);
        validate_health_response(&response, RpcResponseKind::VerifyState)
            .expect("unrepresentable doubled bound should saturate at the response type maximum");
    }

    fn valid_atomic_swap_quote_request() -> RpcRequest {
        let swap = valid_signed_atomic_swap();
        atomic_swap_fee_quote_request(
            "atomic-quote",
            swap.unsigned.rfq_hash,
            swap.unsigned.market_envelope_hash,
            swap.unsigned.nav_epoch,
            swap.unsigned.expires_at_height,
            swap.unsigned.swap_nonce,
            swap.unsigned.leg_0.owner,
            swap.unsigned.leg_0.recipient,
            swap.unsigned.leg_0.issuer,
            swap.unsigned.leg_0.asset_id,
            swap.unsigned.leg_0.amount,
            swap.unsigned.leg_1.owner,
            swap.unsigned.leg_1.recipient,
            swap.unsigned.leg_1.issuer,
            swap.unsigned.leg_1.asset_id,
            swap.unsigned.leg_1.amount,
        )
    }

    fn valid_atomic_swap_quote_response() -> RpcResponse {
        let swap = valid_signed_atomic_swap();
        success_response(
            "atomic-quote",
            &json!({
                "schema": ATOMIC_SWAP_FEE_QUOTE_SCHEMA,
                "transaction_kind": ATOMIC_SWAP_TRANSACTION_KIND,
                "parent_height": 7,
                "parent_hash": "01".repeat(48),
                "parent_state_root": "02".repeat(48),
                "quote_height": 8,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "atomic_swap_weight_bytes": 4096,
                "leg_0": {
                    "owner": swap.unsigned.leg_0.owner,
                    "sender_balance": 1000,
                    "sender_sequence": 2,
                    "sequence": 3,
                    "mempool_pending_for_owner": 0,
                    "base_atomic_swap_fee": 22,
                    "state_expansion_fee": 0,
                    "minimum_fee": 22,
                    "sender_balance_after_fee": 978,
                    "sender_meets_reserve_after_fee": true
                },
                "leg_1": {
                    "owner": swap.unsigned.leg_1.owner,
                    "sender_balance": 1000,
                    "sender_sequence": 4,
                    "sequence": 5,
                    "mempool_pending_for_owner": 0,
                    "base_atomic_swap_fee": 22,
                    "state_expansion_fee": 0,
                    "minimum_fee": 22,
                    "sender_balance_after_fee": 978,
                    "sender_meets_reserve_after_fee": true
                },
                "unsigned_transaction": swap.unsigned
            }),
            vec![],
        )
        .expect("atomic quote response")
    }

    fn valid_atomic_swap_finality_response() -> RpcResponse {
        let hex96 = "01".repeat(48);
        let swap = valid_signed_atomic_swap();
        let tx_id = atomic_swap_transaction_tx_id(&swap);
        let receipt = json!({
            "tx_id": tx_id,
            "accepted": true,
            "code": "accepted",
            "message": "atomic swap applied",
            "fee_charged": 44,
            "fee_burned": 44,
            "minimum_fee": 44,
            "account_reserve": 10,
            "atomic_swap_legs": [
                {
                    "owner": swap.unsigned.leg_0.owner,
                    "recipient": swap.unsigned.leg_0.recipient,
                    "asset_id": swap.unsigned.leg_0.asset_id,
                    "amount": swap.unsigned.leg_0.amount,
                    "fee_charged": swap.unsigned.leg_0.fee,
                    "pre_sequence": 2,
                    "post_sequence": 3
                },
                {
                    "owner": swap.unsigned.leg_1.owner,
                    "recipient": swap.unsigned.leg_1.recipient,
                    "asset_id": swap.unsigned.leg_1.asset_id,
                    "amount": swap.unsigned.leg_1.amount,
                    "fee_charged": swap.unsigned.leg_1.fee,
                    "pre_sequence": 4,
                    "post_sequence": 5
                }
            ]
        });
        let finality = json!({
            "schema": "postfiat-tx-finality-v1",
            "proof_id": hex96,
            "chain_id": "postfiat-local",
            "genesis_hash": swap.unsigned.genesis_hash,
            "protocol_version": 1,
            "tx_id": tx_id,
            "confirmed": true,
            "verification_mode": "full-block-replay",
            "receipt": receipt,
            "receipt_index": 0,
            "receipt_count": 1,
            "block": {
                "header": {
                    "height": 8,
                    "parent_hash": "01".repeat(48),
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "block_hash": hex96,
                    "state_root": hex96,
                    "receipt_count": 1,
                    "certificate_id": hex96,
                    "certificate": valid_block_certificate(&hex96)
                },
                "receipt_ids": [tx_id]
            },
            "block_log_verified": true,
            "block_count": 8,
            "tip_hash": hex96,
            "tip_state_root": hex96
        });
        success_response(
            "atomic-finality",
            &json!({
                "schema": ATOMIC_SWAP_FINALITY_SCHEMA,
                "tx_id": tx_id,
                "finality": finality,
                "round_report_file": "/tmp/atomic-round.json",
                "artifact_dir": "/tmp/atomic-round",
                "readiness_wait_ms": 1.0,
                "mempool_submit_ms": 2.0,
                "mempool_batch_ms": 3.0,
                "certified_round_ms": 4.0,
                "total_ms": 10.0,
                "certified_sends_deferred": true,
                "round_ok": false
            }),
            vec![],
        )
        .expect("atomic finality response")
    }

    fn atomic_swap_submit_response(
        id: &str,
        swap: &postfiat_types::SignedAtomicSwapTransaction,
    ) -> RpcResponse {
        success_response(
            id,
            &json!({
                "tx_id": atomic_swap_transaction_tx_id(swap),
                "transaction": swap,
            }),
            vec![],
        )
        .expect("atomic submit response")
    }

    #[test]
    fn atomic_swap_quote_and_finality_wrappers_validate_and_decode() {
        let quote_request = valid_atomic_swap_quote_request();
        let quote = valid_atomic_swap_quote_response();
        let summary =
            decode_atomic_swap_fee_quote_summary(&quote, &quote_request).expect("atomic quote summary");
        assert_eq!(summary.leg_0.minimum_fee, 22);
        assert_eq!(summary.leg_1.sequence, 5);
        let pinned = mempool_submit_signed_atomic_swap_transaction_finality_from_quote_request(
            "atomic-pinned",
            serde_json::to_string(&valid_signed_atomic_swap()).expect("signed swap json"),
            &summary,
            Some(5_000),
        );
        assert_eq!(
            pinned.params["proxy_required_current_height"],
            json!(summary.parent_height)
        );
        assert_eq!(
            pinned.params["proxy_required_parent_hash"],
            json!(summary.parent_hash)
        );
        assert_eq!(
            pinned.params["proxy_required_state_root"],
            json!(summary.parent_state_root)
        );

        let mut finality_request = pinned;
        finality_request.id = "atomic-finality".to_string();
        let finality = valid_atomic_swap_finality_response();
        let summary = decode_atomic_swap_finality_summary(&finality, &finality_request)
            .expect("atomic finality summary");
        assert!(summary.accepted);
        assert_eq!(summary.receipt_code, "accepted");
    }

    #[test]
    fn atomic_swap_response_validation_rejects_inconsistent_quote_and_finality_tx_id() {
        let mut quote = valid_atomic_swap_quote_response();
        quote.result.as_mut().expect("quote result")["leg_0"]["minimum_fee"] = json!(23);
        assert!(matches!(
            validate_response_kind(&quote, RpcResponseKind::AtomicSwapFeeQuote),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "leg_0.minimum_fee"
        ));

        let mut pending_quote = valid_atomic_swap_quote_response();
        pending_quote.result.as_mut().expect("quote result")["leg_1"]["mempool_pending_for_owner"] =
            json!(1);
        assert!(matches!(
            validate_response_kind(&pending_quote, RpcResponseKind::AtomicSwapFeeQuote),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "leg_1.mempool_pending_for_owner"
        ));

        let mut skipped_parent_quote = valid_atomic_swap_quote_response();
        skipped_parent_quote.result.as_mut().expect("quote result")["quote_height"] = json!(9);
        assert!(matches!(
            validate_response_kind(&skipped_parent_quote, RpcResponseKind::AtomicSwapFeeQuote),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "quote_height"
        ));

        let mut finality = valid_atomic_swap_finality_response();
        let other_tx_id = "02".repeat(48);
        let finality_result = finality.result.as_mut().expect("finality result");
        finality_result["finality"]["tx_id"] = json!(other_tx_id);
        finality_result["finality"]["receipt"]["tx_id"] = json!(other_tx_id);
        finality_result["finality"]["block"]["receipt_ids"][0] = json!(other_tx_id);
        assert!(matches!(
            validate_response_kind(
                &finality,
                RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "finality.tx_id"
        ));
    }

    #[test]
    fn atomic_swap_quote_decode_rejects_response_substitution_before_signing() {
        let request = valid_atomic_swap_quote_request();
        let mut substituted = valid_atomic_swap_quote_response();
        substituted.result.as_mut().expect("quote result")["unsigned_transaction"]["leg_0"]["amount"] =
            json!(20_001);
        substituted.result.as_mut().expect("quote result")["leg_0"]["minimum_fee"] = json!(22);
        assert!(matches!(
            decode_atomic_swap_fee_quote_summary(&substituted, &request),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "unsigned_transaction.leg_0_amount"
        ));

        let mut wrong_id = valid_atomic_swap_quote_response();
        wrong_id.id = "another-request".to_string();
        assert!(matches!(
            decode_atomic_swap_fee_quote_summary(&wrong_id, &request),
            Err(RpcResponseValidationError::UnexpectedId { .. })
        ));
    }

    #[test]
    fn atomic_swap_finality_decode_rejects_a_different_valid_transaction() {
        let swap = valid_signed_atomic_swap();
        let request = mempool_submit_signed_atomic_swap_transaction_finality_request(
            "atomic-finality",
            serde_json::to_string(&swap).expect("signed swap JSON"),
            7,
            &"02".repeat(48),
            &"01".repeat(48),
            None,
        );
        let mut substituted = valid_atomic_swap_finality_response();
        substituted.result.as_mut().expect("finality result")["finality"]["receipt"]
            ["atomic_swap_legs"][0]["amount"] = json!(swap.unsigned.leg_0.amount + 1);
        assert!(matches!(
            decode_atomic_swap_finality_summary(&substituted, &request),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "finality.receipt.atomic_swap_legs.leg_0"
        ));

        let mut wrong_height = valid_atomic_swap_finality_response();
        wrong_height.result.as_mut().expect("finality result")["finality"]["block"]["header"]
            ["height"] = json!(9);
        wrong_height.result.as_mut().expect("finality result")["finality"]["block_count"] = json!(9);
        assert!(matches!(
            decode_atomic_swap_finality_summary(&wrong_height, &request),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "finality.block.header.height"
        ));

        let mut wrong_parent = valid_atomic_swap_finality_response();
        wrong_parent.result.as_mut().expect("finality result")["finality"]["block"]["header"]
            ["parent_hash"] = json!("03".repeat(48));
        assert!(matches!(
            decode_atomic_swap_finality_summary(&wrong_parent, &request),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "finality.block.header.parent_hash"
        ));

        let mut wrong_id = valid_atomic_swap_finality_response();
        wrong_id.id = "another-request".to_string();
        assert!(matches!(
            decode_atomic_swap_finality_summary(&wrong_id, &request),
            Err(RpcResponseValidationError::UnexpectedId { .. })
        ));
    }

    #[test]
    fn atomic_swap_raw_submit_decode_rejects_a_different_valid_transaction() {
        let submitted = valid_signed_atomic_swap();
        let request = mempool_submit_signed_atomic_swap_transaction_json_request(
            "atomic-submit",
            serde_json::to_string(&submitted).expect("submitted swap JSON"),
        );
        let accepted = atomic_swap_submit_response("atomic-submit", &submitted);
        let decoded = decode_atomic_swap_mempool_submit_entry(&accepted, &request)
            .expect("request-bound atomic submit response");
        assert_eq!(decoded.transaction, submitted);

        let mut other = valid_signed_atomic_swap();
        other.unsigned.swap_nonce = "ab".repeat(48);
        let substituted = atomic_swap_submit_response("atomic-submit", &other);
        validate_response_kind(
            &substituted,
            RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction,
        )
        .expect("substituted response remains structurally valid");
        assert!(matches!(
            decode_atomic_swap_mempool_submit_entry(&substituted, &request),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "transaction"
        ));

        let wrong_id = atomic_swap_submit_response("another-request", &submitted);
        assert!(matches!(
            decode_atomic_swap_mempool_submit_entry(&wrong_id, &request),
            Err(RpcResponseValidationError::UnexpectedId { .. })
        ));
    }

    #[test]
    fn atomic_swap_finality_requires_two_leg_receipts_and_mempool_ids_are_hash_bound() {
        for malformed in ["missing", "null", "one"] {
            let mut response = valid_atomic_swap_finality_response();
            let receipt = response
                .result
                .as_mut()
                .expect("finality result")
                .get_mut("finality")
                .and_then(|value| value.get_mut("receipt"))
                .and_then(Value::as_object_mut)
                .expect("finality receipt object");
            match malformed {
                "missing" => {
                    receipt.remove("atomic_swap_legs");
                }
                "null" => {
                    receipt.insert("atomic_swap_legs".to_string(), Value::Null);
                }
                "one" => {
                    let one_leg = receipt["atomic_swap_legs"][0].clone();
                    receipt.insert("atomic_swap_legs".to_string(), json!([one_leg]));
                }
                _ => unreachable!(),
            }
            assert!(
                validate_response_kind(
                    &response,
                    RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality
                )
                .is_err(),
                "{malformed} atomic leg receipts must be rejected"
            );
        }

        let swap = valid_signed_atomic_swap();
        let forged_entry = success_response(
            "forged-atomic-entry",
            &json!({
                "tx_id": "ff".repeat(48),
                "transaction": swap
            }),
            vec![],
        )
        .expect("forged atomic entry response");
        assert!(matches!(
            validate_response_kind(
                &forged_entry,
                RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "tx_id"
        ));
    }

    #[test]
    fn atomic_swap_finality_rejects_failed_undeferred_round_and_nonaccepted_receipt() {
        let mut failed_round = valid_atomic_swap_finality_response();
        let failed_round_result = failed_round.result.as_mut().expect("finality result");
        failed_round_result["certified_sends_deferred"] = json!(false);
        failed_round_result["round_ok"] = json!(false);
        assert!(matches!(
            validate_response_kind(
                &failed_round,
                RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "round_ok"
        ));

        let mut rejected = valid_atomic_swap_finality_response();
        let rejected_receipt =
            &mut rejected.result.as_mut().expect("finality result")["finality"]["receipt"];
        rejected_receipt["accepted"] = json!(false);
        rejected_receipt["code"] = json!("rejected");
        assert!(matches!(
            validate_response_kind(
                &rejected,
                RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "accepted"
        ));
    }

    #[test]
    fn atomic_swap_finality_binds_outer_fee_accounting_to_leg_receipts() {
        for (field, value) in [
            ("fee_charged", 43),
            ("fee_burned", 43),
            ("minimum_fee", 45),
            ("account_reserve", 0),
        ] {
            let mut response = valid_atomic_swap_finality_response();
            response.result.as_mut().expect("finality result")["finality"]["receipt"][field] =
                json!(value);
            assert!(matches!(
                validate_response_kind(
                    &response,
                    RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality
                ),
                Err(RpcResponseValidationError::InvalidResult { field: invalid, .. })
                    if invalid == field
            ));
        }
    }

    #[test]
    fn generic_atomic_swap_submit_mempool_archive_and_account_history_validate() {
        let hex96 = "01".repeat(48);
        let swap = valid_signed_atomic_swap();
        let atomic_tx_id = hash_hex(
            postfiat_types::ATOMIC_SWAP_TRANSACTION_TX_ID_DOMAIN,
            &swap.tx_id_preimage_bytes(),
        );
        let entry = json!({"tx_id": atomic_tx_id, "transaction": swap});
        let submit = success_response("atomic-submit", &entry, vec![]).expect("submit response");
        validate_response_kind(
            &submit,
            RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction,
        )
        .expect("atomic submit response");

        let mempool = success_response(
            "atomic-mempool",
            &json!({"pending": [], "pending_atomic_swaps": [entry]}),
            vec![],
        )
        .expect("mempool response");
        validate_response_kind(&mempool, RpcResponseKind::MempoolStatus)
            .expect("atomic mempool response");

        let batch = json!({
            "batch_id": hex96,
            "transactions": [],
            "atomic_swap_transactions": [valid_signed_atomic_swap()]
        });
        let batch_response = success_response("atomic-batch", &batch, vec![]).expect("batch response");
        validate_response_kind(&batch_response, RpcResponseKind::MempoolBatch)
            .expect("atomic batch response");

        let archive = success_response(
            "atomic-archive",
            &json!([{
                "batch_kind": "transparent",
                "batch_id": hex96,
                "payload_hash": hex96,
                "payload_json": serde_json::to_string(&batch).expect("batch json")
            }]),
            vec![],
        )
        .expect("archive response");
        validate_response_kind(&archive, RpcResponseKind::BatchArchive)
            .expect("atomic archive response");

        let owner = valid_signed_atomic_swap().unsigned.leg_0.owner;
        let account_tx = success_response(
            "atomic-account-tx",
            &json!({
                "schema": "postfiat-account-tx-v1",
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "address": owner,
                "from_height": null,
                "to_height": null,
                "scan_limit": 10,
                "scanned_block_count": 1,
                "archive_lookup_count": 1,
                "truncated": false,
                "row_count": 1,
                "rows": [{
                    "tx_id": hex96,
                    "block_height": 1,
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "transaction_index": 0,
                    "transaction_kind": ATOMIC_SWAP_TRANSACTION_KIND,
                    "from": owner,
                    "to": valid_signed_atomic_swap().unsigned.leg_0.recipient,
                    "amount": 20_000,
                    "fee": 22,
                    "sequence": 3,
                    "memo_hash": null,
                    "asset_id": "10".repeat(48),
                    "issuer": format!("pf{}", "03".repeat(20)),
                    "tx_role": "leg_0",
                    "accepted": true,
                    "receipt_code": "accepted"
                }]
            }),
            vec![],
        )
        .expect("account tx response");
        validate_response_kind(&account_tx, RpcResponseKind::AccountTx)
            .expect("atomic account tx response");
    }

    #[test]
    fn wallet_flow_summary_helpers_decode_validated_responses() {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let other_hex96 = "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111";

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
                "sequence": 7,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 6,
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
                "recipient_balance_after_amount": null,
                "recipient_meets_reserve_after_transfer": true
            }),
            vec![],
        )
        .expect("transfer fee quote response");
        let quote_summary =
            decode_transfer_fee_quote_summary(&fee_quote).expect("fee quote summary");
        assert_eq!(quote_summary.chain_id, "postfiat-local");
        assert_eq!(quote_summary.amount, 25);
        assert_eq!(quote_summary.sequence, 7);
        assert_eq!(quote_summary.sequence_source, "explicit");
        assert_eq!(quote_summary.minimum_fee, 32);
        assert_eq!(quote_summary.sender_balance_after_amount_and_fee, Some(43));
        assert_eq!(quote_summary.recipient_balance_after_amount, None);

        let asset_fee_quote = success_response(
            "asset-fee-quote-1",
            &json!({
                "schema": ASSET_FEE_QUOTE_SCHEMA,
                "transaction_kind": ASSET_CREATE_TRANSACTION_KIND,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": "pf1-sender",
                "sequence": 8,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 7,
                "mempool_pending_for_sender": 1,
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
        let asset_quote_summary =
            decode_asset_fee_quote_summary(&asset_fee_quote).expect("asset fee quote summary");
        assert_eq!(
            asset_quote_summary.transaction_kind,
            ASSET_CREATE_TRANSACTION_KIND
        );
        assert_eq!(asset_quote_summary.sequence, 8);
        assert_eq!(asset_quote_summary.minimum_fee, 42);
        assert_eq!(asset_quote_summary.sender_balance_after_fee, Some(58));

        let escrow_fee_quote = success_response(
            "escrow-fee-quote-1",
            &json!({
                "schema": ESCROW_FEE_QUOTE_SCHEMA,
                "transaction_kind": ESCROW_CREATE_TRANSACTION_KIND,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": "pf1-sender",
                "sequence": 9,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 8,
                "mempool_pending_for_sender": 1,
                "base_escrow_fee": 22,
                "state_expansion_fee": 10,
                "minimum_fee": 32,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "escrow_weight_bytes": 11000,
                "sender_balance_after_fee": 68,
                "sender_meets_reserve_after_fee": true,
                "operation": {
                    "operation": ESCROW_CREATE_TRANSACTION_KIND,
                    "owner": "pf1-sender",
                    "recipient": "pf1-recipient",
                    "asset_id": "PFT",
                    "amount": 20,
                    "condition": "secret",
                    "cancel_after": 5
                }
            }),
            vec![],
        )
        .expect("escrow fee quote response");
        let escrow_quote_summary =
            decode_escrow_fee_quote_summary(&escrow_fee_quote).expect("escrow fee quote summary");
        assert_eq!(
            escrow_quote_summary.transaction_kind,
            ESCROW_CREATE_TRANSACTION_KIND
        );
        assert_eq!(escrow_quote_summary.sequence, 9);
        assert_eq!(escrow_quote_summary.minimum_fee, 32);

        let signed_transfer = valid_signed_transfer(other_hex96);
        let mempool_submit = success_response(
            "mempool-submit-1",
            &json!({
                "tx_id": hex96,
                "transfer": signed_transfer
            }),
            vec![],
        )
        .expect("mempool submit response");
        let submit_summary = decode_mempool_submit_transfer_summary(&mempool_submit)
            .expect("mempool submit summary");
        assert_eq!(submit_summary.tx_id, hex96);
        assert_eq!(submit_summary.from, "pffaucet");
        assert_eq!(submit_summary.to, "pfrecipient");
        assert_eq!(submit_summary.amount, 42);
        assert_eq!(submit_summary.fee, 1);
        assert_eq!(submit_summary.sequence, 0);
        assert_eq!(submit_summary.algorithm_id, ML_DSA_65_ALGORITHM);

        let signed_submit_summary = decode_mempool_submit_signed_transfer_summary(&mempool_submit)
            .expect("signed mempool submit summary");
        assert_eq!(signed_submit_summary, submit_summary);

        let submit_summary_json =
            serde_json::to_value(&submit_summary).expect("submit summary json");
        assert!(submit_summary_json.get("public_key_hex").is_none());
        assert!(submit_summary_json.get("signature_hex").is_none());

        let tx_poll = tx_finality_request_from_submit("tx-poll-1", &submit_summary);
        assert_eq!(tx_poll.method, METHOD_TX);
        assert_eq!(tx_poll.params, json!({"tx_id": hex96}));

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
                "block_log_verified": true,
                "block_count": 1,
                "tip_hash": hex96,
                "tip_state_root": hex96
            }),
            vec![],
        )
        .expect("tx response");
        let finality_summary = decode_tx_finality_summary(&tx).expect("tx finality summary");
        assert_eq!(finality_summary.tx_id, hex96);
        assert!(finality_summary.accepted);
        assert_eq!(finality_summary.receipt_code, "accepted");
        assert_eq!(finality_summary.block_height, 1);
        assert_eq!(finality_summary.block_hash, hex96);
        assert_eq!(finality_summary.certificate_id, hex96);
        assert_eq!(finality_summary.registry_root, hex96);
        assert_eq!(finality_summary.quorum, 1);
        assert_eq!(finality_summary.validator_count, 1);
        assert_eq!(finality_summary.vote_count, 1);

        let mut bad_tx = tx;
        bad_tx.result.as_mut().expect("result")["receipt"]["tx_id"] = json!(other_hex96);
        assert!(matches!(
            decode_tx_finality_summary(&bad_tx),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "receipt.tx_id"
        ));
    }

    #[test]
    fn wallet_sign_owned_transfer_order_signs_fastpay_order() {
        let backup = wallet_backup_from_master_seed(
            "postfiat-wallet-sdk",
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
            0,
        )
        .expect("wallet backup");
        let identity = wallet_identity_from_backup(&backup).expect("identity");
        let order = OwnedTransferOrder {
            domain: postfiat_types::OwnedCertificateDomain {
                schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
                chain_id: "postfiat-wallet-sdk".to_string(),
                genesis_hash: "ab".repeat(48),
                protocol_version: 1,
                registry_id: "cd".repeat(48),
            },
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "ab".repeat(32),
                version: 1,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: identity.public_key_hex.clone(),
                value: 99,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 42,
            memos: Vec::new(),
        };

        let signed = wallet_sign_owned_transfer_order(&backup, order.clone())
            .expect("owned-transfer signature");
        let public_key =
            crypto_hex_to_bytes(&signed.owner_pubkey_hex).expect("owner public key bytes");
        let signature =
            crypto_hex_to_bytes(&signed.owner_signature_hex).expect("owner signature bytes");

        assert_eq!(signed.owner_pubkey_hex, identity.public_key_hex);
        assert_eq!(signed.order, order);
        assert!(ml_dsa_65_verify_with_context(
            &public_key,
            &owned_transfer_signing_bytes(&signed.order),
            &signature,
            OWNED_TRANSFER_CONTEXT,
        ));
    }

    #[test]
    fn wallet_refuses_to_sign_fastpay_order_for_foreign_chain() {
        let backup = wallet_backup_from_master_seed(
            "postfiat-wallet-sdk",
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
            0,
        )
        .expect("wallet backup");
        let order = OwnedTransferOrder {
            domain: postfiat_types::OwnedCertificateDomain {
                schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
                chain_id: "postfiat-foreign-chain".to_string(),
                genesis_hash: "ab".repeat(48),
                protocol_version: 1,
                registry_id: "cd".repeat(48),
            },
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "ab".repeat(32),
                version: 1,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: "ef".repeat(32),
                value: 99,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 42,
            memos: Vec::new(),
        };

        let error = wallet_sign_owned_transfer_order(&backup, order)
            .expect_err("foreign-chain FastPay order must fail closed");
        assert!(error.to_string().contains("does not match wallet chain"));
    }

    #[test]
    fn wallet_sign_owned_unwrap_order_signs_fastpay_unwrap_order() {
        let backup = wallet_backup_from_master_seed(
            "postfiat-wallet-sdk",
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
            0,
        )
        .expect("wallet backup");
        let identity = wallet_identity_from_backup(&backup).expect("identity");
        let order = postfiat_types::OwnedUnwrapOrder {
            domain: postfiat_types::OwnedCertificateDomain {
                schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
                chain_id: "postfiat-wallet-sdk".to_string(),
                genesis_hash: "ab".repeat(48),
                protocol_version: 1,
                registry_id: "cd".repeat(48),
            },
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "cd".repeat(32),
                version: 2,
            }],
            to_address: identity.address.clone(),
            amount: 25,
            asset: "PFT".to_string(),
            fee: 1,
            nonce: 43,
            memos: Vec::new(),
        };

        let signed =
            wallet_sign_owned_unwrap_order(&backup, order.clone()).expect("owned-unwrap signature");
        let public_key =
            crypto_hex_to_bytes(&signed.owner_pubkey_hex).expect("owner public key bytes");
        let signature =
            crypto_hex_to_bytes(&signed.owner_signature_hex).expect("owner signature bytes");

        assert_eq!(signed.owner_pubkey_hex, identity.public_key_hex);
        assert_eq!(signed.order, order);
        assert!(ml_dsa_65_verify_with_context(
            &public_key,
            &owned_unwrap_signing_bytes(&signed.order),
            &signature,
            OWNED_UNWRAP_CONTEXT,
        ));
    }

    #[test]
    fn wallet_signs_fastpay_v3_only_against_exact_live_recovery_capability() {
        let backup = wallet_backup_from_master_seed(
            "postfiat-wallet-sdk",
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
            0,
        )
        .expect("wallet backup");
        let identity = wallet_identity_from_backup(&backup).expect("identity");
        let domain = postfiat_types::OwnedCertificateDomain {
            schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
            chain_id: backup.chain_id.clone(),
            genesis_hash: "ab".repeat(48),
            protocol_version: 1,
            registry_id: "cd".repeat(48),
        };
        let capabilities = postfiat_types::FastPayRecoveryCapabilitiesV1 {
            schema: postfiat_types::FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1.to_string(),
            domain: domain.clone(),
            committee_epoch: 7,
            current_height: 100,
            validator_count: 4,
            quorum: 3,
            policy: postfiat_types::FastPayRecoveryPolicyV1 {
                schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
                activation_height: 90,
                max_validity_blocks: 20,
                max_recovery_blocks: 20,
            },
        };
        let recovery = postfiat_types::FastPayOrderRecoveryV1 {
            schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
            committee_epoch: 7,
            lock_id: "00".repeat(48),
            valid_from_height: 100,
            expires_at_height: 110,
            recovery_closes_at_height: 120,
        };
        let mut transfer = postfiat_types::OwnedTransferOrderV3 {
            domain: domain.clone(),
            recovery: recovery.clone(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "ef".repeat(32),
                version: 2,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: identity.public_key_hex.clone(),
                value: 49,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 7,
            memos: Vec::new(),
        };
        transfer.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&transfer);
        let signed_transfer =
            wallet_sign_owned_transfer_order_v3(&backup, transfer.clone(), &capabilities)
                .expect("sign FastPay v3 transfer");
        assert_eq!(signed_transfer.order, transfer);
        assert_eq!(signed_transfer.owner_pubkey_hex, identity.public_key_hex);
        assert!(ml_dsa_65_verify_with_context(
            &crypto_hex_to_bytes(&signed_transfer.owner_pubkey_hex).expect("owner key"),
            &postfiat_execution::owned_transfer_v3_signing_bytes(&signed_transfer.order),
            &crypto_hex_to_bytes(&signed_transfer.owner_signature_hex).expect("owner signature"),
            postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
        ));

        let mut unwrap = postfiat_types::OwnedUnwrapOrderV3 {
            domain,
            recovery,
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "12".repeat(32),
                version: 3,
            }],
            to_address: identity.address,
            amount: 49,
            asset: "PFT".to_string(),
            fee: 1,
            nonce: 8,
            memos: Vec::new(),
        };
        unwrap.recovery.lock_id = postfiat_types::fastpay_unwrap_lock_id_v1(&unwrap);
        let signed_unwrap =
            wallet_sign_owned_unwrap_order_v3(&backup, unwrap.clone(), &capabilities)
                .expect("sign FastPay v3 unwrap");
        assert_eq!(signed_unwrap.order, unwrap);
        assert!(ml_dsa_65_verify_with_context(
            &crypto_hex_to_bytes(&signed_unwrap.owner_pubkey_hex).expect("unwrap owner key"),
            &postfiat_execution::owned_unwrap_v3_signing_bytes(&signed_unwrap.order),
            &crypto_hex_to_bytes(&signed_unwrap.owner_signature_hex)
                .expect("unwrap owner signature"),
            postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
        ));

        let mut wrong_capability = capabilities;
        wrong_capability.committee_epoch = 8;
        let error = wallet_sign_owned_transfer_order_v3(
            &backup,
            signed_transfer.order,
            &wrong_capability,
        )
        .expect_err("wallet must not sign against a different live committee");
        assert!(error.to_string().contains("live recovery capability"));
    }

    #[test]
    fn wallet_verifies_fastpay_apply_ack_signature_and_rejects_tampering() {
        let validator = ml_dsa_65_keygen_from_seed(&[9_u8; 32]);
        let validator_public_key_hex = crypto_bytes_to_hex(&validator.public_key);
        let mut acknowledgement = postfiat_types::FastPayApplyAckV1 {
            schema: postfiat_types::FASTPAY_APPLY_ACK_SCHEMA_V1.to_string(),
            domain: postfiat_types::OwnedCertificateDomain {
                schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
                chain_id: "postfiat-wallet-sdk".to_string(),
                genesis_hash: "ab".repeat(48),
                protocol_version: 1,
                registry_id: "cd".repeat(48),
            },
            committee_epoch: 7,
            lock_id: "11".repeat(48),
            order_digest: "22".repeat(48),
            certificate_digest: "33".repeat(48),
            terminal_state_digest: "44".repeat(48),
            validator_id: "validator-0".to_string(),
            signature_hex: String::new(),
        };
        let signing_bytes = postfiat_execution::fastpay_apply_ack_signing_bytes_v1(
            &acknowledgement,
        )
        .expect("acknowledgement signing bytes");
        acknowledgement.signature_hex = crypto_bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &validator.private_key,
                &signing_bytes,
                postfiat_execution::FASTPAY_APPLY_ACK_CONTEXT_V1,
            )
            .expect("sign acknowledgement"),
        );

        wallet_verify_fastpay_apply_ack_v1(&acknowledgement, &validator_public_key_hex)
            .expect("valid acknowledgement");

        acknowledgement.terminal_state_digest = "55".repeat(48);
        let error = wallet_verify_fastpay_apply_ack_v1(
            &acknowledgement,
            &validator_public_key_hex,
        )
        .expect_err("tampered acknowledgement must fail");
        assert!(error.to_string().contains("signature is invalid"));
    }

    #[test]
    fn account_and_receipt_summary_helpers_decode_validated_responses() {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let account = success_response(
            "account-1",
            &json!({
                "address": "pf1-test",
                "balance": 25,
                "sequence": 7,
                "public_key_hex": hex96
            }),
            vec![],
        )
        .expect("account response");
        let account_summary = decode_account_summary(&account).expect("account summary");
        assert_eq!(
            account_summary,
            AccountSummary {
                address: "pf1-test".to_string(),
                balance: 25,
                sequence: 7,
                public_key_hex: Some(hex96.to_string()),
            }
        );

        let unbound_account = success_response(
            "account-2",
            &json!({
                "address": "pf1-unbound",
                "balance": 10,
                "sequence": 0,
                "public_key_hex": null
            }),
            vec![],
        )
        .expect("unbound account response");
        let unbound_summary =
            decode_account_summary(&unbound_account).expect("unbound account summary");
        assert_eq!(unbound_summary.public_key_hex, None);

        let receipts = success_response(
            "receipts-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": true,
                    "code": "accepted",
                    "message": "transfer applied"
                },
                {
                    "tx_id": "local-sim",
                    "accepted": false,
                    "code": "duplicate_witness",
                    "message": "witness already processed"
                }
            ]),
            vec![],
        )
        .expect("receipts response");
        let receipt_summaries = decode_receipt_summaries(&receipts).expect("receipt summaries");
        assert_eq!(receipt_summaries.len(), 2);
        assert!(receipt_summaries[0].accepted);
        assert_eq!(receipt_summaries[0].tx_id, hex96);
        assert_eq!(receipt_summaries[1].code, "duplicate_witness");

        let bad_account = success_response(
            "account-3",
            &json!({
                "address": "pf1-bad",
                "balance": 25,
                "sequence": 7,
                "public_key_hex": "AA"
            }),
            vec![],
        )
        .expect("bad account response");
        assert!(matches!(
            decode_account_summary(&bad_account),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "public_key_hex"
        ));
    }

    #[test]
    fn wallet_sdk_creates_identity_and_signs_quoted_transfer_without_key_file() {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let master_seed_hex = "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F";
        let backup = wallet_backup_from_master_seed("postfiat-wallet-sdk", master_seed_hex, 2)
            .expect("wallet backup");
        assert_eq!(
            backup.master_seed_hex,
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        );

        let identity = wallet_identity_from_backup(&backup).expect("wallet identity");
        assert_eq!(identity.algorithm_id, ML_DSA_65_ALGORITHM);
        assert_eq!(identity.chain_id, "postfiat-wallet-sdk");
        assert_eq!(identity.account_index, 2);
        assert!(identity.private_key_material_redacted);
        assert!(identity.address.starts_with("pf"));

        let restored_identity = wallet_identity_from_backup(&backup).expect("restored identity");
        assert_eq!(restored_identity, identity);

        let fee_quote = success_response(
            "fee-quote-1",
            &json!({
                "schema": TRANSFER_FEE_QUOTE_SCHEMA,
                "chain_id": "postfiat-wallet-sdk",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "from": identity.address,
                "to": "pf1-recipient",
                "amount": 25,
                "sequence": 7,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 6,
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
        let quote_summary =
            decode_transfer_fee_quote_summary(&fee_quote).expect("fee quote summary");
        let signed = wallet_sign_transfer_from_quote(&backup, &quote_summary)
            .expect("signed quoted transfer");
        assert_eq!(signed.unsigned.chain_id, quote_summary.chain_id);
        assert_eq!(signed.unsigned.genesis_hash, quote_summary.genesis_hash);
        assert_eq!(
            signed.unsigned.protocol_version,
            quote_summary.protocol_version
        );
        assert_eq!(signed.unsigned.from, quote_summary.from);
        assert_eq!(signed.unsigned.to, quote_summary.to);
        assert_eq!(signed.unsigned.amount, quote_summary.amount);
        assert_eq!(signed.unsigned.fee, quote_summary.minimum_fee);
        assert_eq!(signed.unsigned.sequence, quote_summary.sequence);
        assert_eq!(signed.algorithm_id, ML_DSA_65_ALGORITHM);
        assert_eq!(signed.public_key_hex, restored_identity.public_key_hex);
        signed.validate().expect("signed transfer validates");

        let public_key = crypto_hex_to_bytes(&signed.public_key_hex).expect("public key bytes");
        let signature = crypto_hex_to_bytes(&signed.signature_hex).expect("signature bytes");
        assert!(ml_dsa_65_verify(
            &public_key,
            &signed.unsigned.signing_bytes(),
            &signature
        ));

        let asset_fee_quote = success_response(
            "asset-fee-quote-1",
            &json!({
                "schema": ASSET_FEE_QUOTE_SCHEMA,
                "transaction_kind": ASSET_CREATE_TRANSACTION_KIND,
                "chain_id": "postfiat-wallet-sdk",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": restored_identity.address.as_str(),
                "sequence": 8,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 7,
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
                    "issuer": restored_identity.address.as_str(),
                    "code": "USD",
                    "version": 1,
                    "precision": 2
                }
            }),
            vec![],
        )
        .expect("asset fee quote response");
        let signed_asset = wallet_sign_asset_transaction_from_quote(&backup, &asset_fee_quote)
            .expect("signed quoted asset transaction");
        assert_eq!(signed_asset.unsigned.chain_id, "postfiat-wallet-sdk");
        assert_eq!(signed_asset.unsigned.genesis_hash, hex96);
        assert_eq!(signed_asset.unsigned.protocol_version, 1);
        assert_eq!(signed_asset.unsigned.source, restored_identity.address);
        assert_eq!(
            signed_asset.unsigned.transaction_kind,
            ASSET_CREATE_TRANSACTION_KIND
        );
        assert_eq!(signed_asset.unsigned.fee, 42);
        assert_eq!(signed_asset.unsigned.sequence, 8);
        assert_eq!(signed_asset.algorithm_id, ML_DSA_65_ALGORITHM);
        assert_eq!(
            signed_asset.public_key_hex,
            restored_identity.public_key_hex
        );
        signed_asset.validate().expect("signed asset validates");

        let asset_public_key =
            crypto_hex_to_bytes(&signed_asset.public_key_hex).expect("asset public key bytes");
        let asset_signature =
            crypto_hex_to_bytes(&signed_asset.signature_hex).expect("asset signature bytes");
        assert!(ml_dsa_65_verify(
            &asset_public_key,
            &signed_asset.unsigned.signing_bytes(),
            &asset_signature
        ));

        let escrow_fee_quote = success_response(
            "escrow-fee-quote-1",
            &json!({
                "schema": ESCROW_FEE_QUOTE_SCHEMA,
                "transaction_kind": ESCROW_CREATE_TRANSACTION_KIND,
                "chain_id": "postfiat-wallet-sdk",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": restored_identity.address.as_str(),
                "sequence": 9,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 8,
                "mempool_pending_for_sender": 0,
                "base_escrow_fee": 22,
                "state_expansion_fee": 10,
                "minimum_fee": 32,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "escrow_weight_bytes": 11000,
                "sender_balance_after_fee": 68,
                "sender_meets_reserve_after_fee": true,
                "operation": {
                    "operation": ESCROW_CREATE_TRANSACTION_KIND,
                    "owner": restored_identity.address.as_str(),
                    "recipient": "pf1-recipient",
                    "asset_id": "PFT",
                    "amount": 20,
                    "condition": "secret",
                    "cancel_after": 5
                }
            }),
            vec![],
        )
        .expect("escrow fee quote response");
        let signed_escrow = wallet_sign_escrow_transaction_from_quote(&backup, &escrow_fee_quote)
            .expect("signed quoted escrow transaction");
        assert_eq!(signed_escrow.unsigned.chain_id, "postfiat-wallet-sdk");
        assert_eq!(signed_escrow.unsigned.genesis_hash, hex96);
        assert_eq!(signed_escrow.unsigned.protocol_version, 1);
        assert_eq!(signed_escrow.unsigned.source, restored_identity.address);
        assert_eq!(
            signed_escrow.unsigned.transaction_kind,
            ESCROW_CREATE_TRANSACTION_KIND
        );
        assert_eq!(signed_escrow.unsigned.fee, 32);
        assert_eq!(signed_escrow.unsigned.sequence, 9);
        match &signed_escrow.unsigned.operation {
            EscrowTransactionOperation::EscrowCreate(operation) => {
                assert_eq!(operation.finish_after, 0);
                assert_eq!(operation.cancel_after, 5);
            }
            other => panic!("expected escrow_create operation, found {other:?}"),
        }
        assert_eq!(signed_escrow.algorithm_id, ML_DSA_65_ALGORITHM);
        assert_eq!(
            signed_escrow.public_key_hex,
            restored_identity.public_key_hex
        );
        signed_escrow.validate().expect("signed escrow validates");

        let escrow_public_key =
            crypto_hex_to_bytes(&signed_escrow.public_key_hex).expect("escrow public key bytes");
        let escrow_signature =
            crypto_hex_to_bytes(&signed_escrow.signature_hex).expect("escrow signature bytes");
        assert!(ml_dsa_65_verify(
            &escrow_public_key,
            &signed_escrow.unsigned.signing_bytes(),
            &escrow_signature
        ));

        let nft_fee_quote = success_response(
            "nft-fee-quote-1",
            &json!({
                "schema": NFT_FEE_QUOTE_SCHEMA,
                "transaction_kind": NFT_MINT_TRANSACTION_KIND,
                "chain_id": "postfiat-wallet-sdk",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": restored_identity.address.as_str(),
                "sequence": 10,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 9,
                "mempool_pending_for_sender": 0,
                "base_nft_fee": 22,
                "state_expansion_fee": 10,
                "minimum_fee": 32,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "nft_weight_bytes": 11000,
                "sender_balance_after_fee": 68,
                "sender_meets_reserve_after_fee": true,
                "issuer_transfer_fee": 0,
                "sender_balance_after_fee_and_issuer_transfer_fee": 68,
                "sender_meets_reserve_after_fee_and_issuer_transfer_fee": true,
                "operation": {
                    "operation": NFT_MINT_TRANSACTION_KIND,
                    "issuer": restored_identity.address.as_str(),
                    "collection_id": "wallet-sdk-collection",
                    "serial": 1,
                    "owner": "pf1-owner",
                    "metadata_hash": "ab".repeat(32),
                    "metadata_uri": "ipfs://postfiat-nft",
                    "flags": NFT_FLAG_TRANSFERABLE,
                    "collection_flags": NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
                    "issuer_transfer_fee": 7
                }
            }),
            vec![],
        )
        .expect("nft fee quote response");
        let signed_nft = wallet_sign_nft_transaction_from_quote(&backup, &nft_fee_quote)
            .expect("signed quoted nft transaction");
        assert_eq!(signed_nft.unsigned.chain_id, "postfiat-wallet-sdk");
        assert_eq!(signed_nft.unsigned.genesis_hash, hex96);
        assert_eq!(signed_nft.unsigned.protocol_version, 1);
        assert_eq!(signed_nft.unsigned.source, restored_identity.address);
        assert_eq!(
            signed_nft.unsigned.transaction_kind,
            NFT_MINT_TRANSACTION_KIND
        );
        assert_eq!(signed_nft.unsigned.fee, 32);
        assert_eq!(signed_nft.unsigned.sequence, 10);
        assert_eq!(signed_nft.algorithm_id, ML_DSA_65_ALGORITHM);
        assert_eq!(signed_nft.public_key_hex, restored_identity.public_key_hex);
        signed_nft.validate().expect("signed nft validates");

        let nft_public_key =
            crypto_hex_to_bytes(&signed_nft.public_key_hex).expect("nft public key bytes");
        let nft_signature =
            crypto_hex_to_bytes(&signed_nft.signature_hex).expect("nft signature bytes");
        assert!(ml_dsa_65_verify(
            &nft_public_key,
            &signed_nft.unsigned.signing_bytes(),
            &nft_signature
        ));

        let offer_fee_quote = success_response(
            "offer-fee-quote-1",
            &json!({
                "schema": OFFER_FEE_QUOTE_SCHEMA,
                "transaction_kind": OFFER_CREATE_TRANSACTION_KIND,
                "chain_id": "postfiat-wallet-sdk",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": restored_identity.address.as_str(),
                "sequence": 11,
                "sequence_source": "explicit",
                "sender_balance": 100,
                "sender_sequence": 10,
                "mempool_pending_for_sender": 0,
                "base_offer_fee": 22,
                "match_fee": 0,
                "state_expansion_fee": 10,
                "estimated_cross_count": 0,
                "max_dex_crosses_per_transaction": 64,
                "will_create_residual_offer": true,
                "offer_object_reserve": 10,
                "minimum_fee": 32,
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
                    "owner": restored_identity.address.as_str(),
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
        let signed_offer = wallet_sign_offer_transaction_from_quote(&backup, &offer_fee_quote)
            .expect("signed quoted offer transaction");
        assert_eq!(signed_offer.unsigned.chain_id, "postfiat-wallet-sdk");
        assert_eq!(signed_offer.unsigned.genesis_hash, hex96);
        assert_eq!(signed_offer.unsigned.protocol_version, 1);
        assert_eq!(signed_offer.unsigned.source, restored_identity.address);
        assert_eq!(
            signed_offer.unsigned.transaction_kind,
            OFFER_CREATE_TRANSACTION_KIND
        );
        assert_eq!(signed_offer.unsigned.fee, 32);
        assert_eq!(signed_offer.unsigned.sequence, 11);
        assert_eq!(signed_offer.algorithm_id, ML_DSA_65_ALGORITHM);
        assert_eq!(
            signed_offer.public_key_hex,
            restored_identity.public_key_hex
        );
        signed_offer.validate().expect("signed offer validates");

        let offer_public_key =
            crypto_hex_to_bytes(&signed_offer.public_key_hex).expect("offer public key bytes");
        let offer_signature =
            crypto_hex_to_bytes(&signed_offer.signature_hex).expect("offer signature bytes");
        assert!(ml_dsa_65_verify(
            &offer_public_key,
            &signed_offer.unsigned.signing_bytes(),
            &offer_signature
        ));

        let mut mismatched_nft_quote = nft_fee_quote.clone();
        mismatched_nft_quote.result.as_mut().expect("result")["source"] = json!("pf1-other-sender");
        let nft_mismatch = wallet_sign_nft_transaction_from_quote(&backup, &mismatched_nft_quote)
            .expect_err("nft quote source mismatch");
        assert!(nft_mismatch
            .message()
            .contains("does not match wallet address"));

        let mut mismatched_offer_quote = offer_fee_quote.clone();
        mismatched_offer_quote.result.as_mut().expect("result")["source"] =
            json!("pf1-other-sender");
        let offer_mismatch =
            wallet_sign_offer_transaction_from_quote(&backup, &mismatched_offer_quote)
                .expect_err("offer quote source mismatch");
        assert!(offer_mismatch
            .message()
            .contains("does not match wallet address"));

        let mut mismatched_asset_quote = asset_fee_quote.clone();
        mismatched_asset_quote.result.as_mut().expect("result")["source"] =
            json!("pf1-other-sender");
        let asset_mismatch =
            wallet_sign_asset_transaction_from_quote(&backup, &mismatched_asset_quote)
                .expect_err("asset quote source mismatch");
        assert!(asset_mismatch
            .message()
            .contains("does not match wallet address"));

        let mut mismatched_quote = quote_summary.clone();
        mismatched_quote.from = "pf1-other-sender".to_string();
        let mismatch = wallet_sign_transfer_from_quote(&backup, &mismatched_quote)
            .expect_err("quote sender mismatch");
        assert!(mismatch.message().contains("does not match wallet address"));

        let wrong_chain = wallet_sign_transfer_from_fields(
            &backup,
            WalletSignTransferFields {
                chain_id: "postfiat-other-chain".to_string(),
                genesis_hash: hex96.to_string(),
                protocol_version: 1,
                to: "pf1-recipient".to_string(),
                amount: 25,
                fee: 32,
                sequence: 7,
            },
        )
        .expect_err("wrong chain rejected");
        assert!(wrong_chain
            .message()
            .contains("does not match wallet backup"));

        assert!(
            wallet_backup_from_master_seed("postfiat-wallet-sdk", "abcd", 0)
                .expect_err("short seed rejected")
                .message()
                .contains("32 bytes")
        );
    }

    #[test]
    fn block_response_validation_rejects_exhausted_height_without_panicking() {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let exhausted_height_response = success_response(
            "blocks-exhausted-height",
            &json!([
                {
                    "header": {
                        "height": u64::MAX,
                        "parent_hash": hex96,
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "block_hash": hex96,
                        "state_root": hex96,
                        "receipt_count": 0,
                        "certificate_id": hex96,
                        "certificate": valid_block_certificate(hex96)
                    },
                    "receipt_ids": []
                },
                {
                    "header": {
                        "height": u64::MAX,
                        "parent_hash": hex96,
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "block_hash": hex96,
                        "state_root": hex96,
                        "receipt_count": 0,
                        "certificate_id": hex96,
                        "certificate": valid_block_certificate(hex96)
                    },
                    "receipt_ids": []
                }
            ]),
            vec![],
        )
        .expect("blocks response");

        assert!(matches!(
            validate_response_kind(&exhausted_height_response, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. })
                if field == "header.height"
        ));
    }

    #[test]
    fn read_response_validation_rejects_bad_shapes_and_private_key_leaks() {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let other_hex96 = "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111";
        assert_eq!(other_hex96.len(), 96);
        let archive_context = BatchArchiveValidationContext {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: hex96.to_string(),
            protocol_version: 1,
        };

        let dirty_account_address = success_response(
            "account-1",
            &json!({
                "address": " pf1-test",
                "balance": 10,
                "sequence": 1
            }),
            vec![],
        )
        .expect("account response");
        assert!(matches!(
            validate_response_kind(&dirty_account_address, RpcResponseKind::Account),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "address"
        ));

        let bad_receipt = success_response(
            "receipts-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": "true",
                    "code": "accepted",
                    "message": "transfer applied"
                }
            ]),
            vec![],
        )
        .expect("receipts response");
        assert!(matches!(
            validate_response_kind(&bad_receipt, RpcResponseKind::Receipts),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "accepted"
        ));

        let bad_receipt_code = success_response(
            "receipts-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": true,
                    "code": "duplicate_transaction",
                    "message": "transfer applied"
                }
            ]),
            vec![],
        )
        .expect("receipts response");
        assert!(matches!(
            validate_response_kind(&bad_receipt_code, RpcResponseKind::Receipts),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "code"
        ));

        let dirty_receipt_message = success_response(
            "receipts-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": true,
                    "code": "accepted",
                    "message": "transfer applied "
                }
            ]),
            vec![],
        )
        .expect("receipts response");
        assert!(matches!(
            validate_response_kind(&dirty_receipt_message, RpcResponseKind::Receipts),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "message"
        ));

        let mismatched_tx_receipt = success_response(
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
                    "tx_id": other_hex96,
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
                "block_log_verified": true,
                "block_count": 1,
                "tip_hash": hex96,
                "tip_state_root": hex96
            }),
            vec![],
        )
        .expect("tx response");
        assert!(matches!(
            validate_response_kind(&mismatched_tx_receipt, RpcResponseKind::Tx),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "receipt.tx_id"
        ));

        let bad_tx_receipt_index = success_response(
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
                "receipt_index": 1,
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
                "block_log_verified": true,
                "block_count": 1,
                "tip_hash": hex96,
                "tip_state_root": hex96
            }),
            vec![],
        )
        .expect("tx response");
        assert!(matches!(
            validate_response_kind(&bad_tx_receipt_index, RpcResponseKind::Tx),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "receipt_index"
        ));

        let bad_block = success_response(
            "blocks-1",
            &json!([
                {
                    "header": {
                        "height": 1,
                        "parent_hash": "genesis",
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "block_hash": "not-a-hash",
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
        assert!(matches!(
            validate_response_kind(&bad_block, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "header.block_hash"
        ));

        let bad_block_receipt_count = success_response(
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
                        "receipt_count": 2,
                        "certificate_id": hex96,
                        "certificate": valid_block_certificate(hex96)
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(&bad_block_receipt_count, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "header.receipt_count"
        ));

        let dirty_block_receipt_id = success_response(
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
                    "receipt_ids": [format!("{hex96}\n")]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(&dirty_block_receipt_id, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "receipt_ids"
        ));

        let mut missing_registry_root_certificate = valid_block_certificate(hex96);
        missing_registry_root_certificate
            .as_object_mut()
            .expect("certificate object")
            .remove("registry_root");
        let missing_registry_root_block = success_response(
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
                        "certificate": missing_registry_root_certificate
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(&missing_registry_root_block, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "registry_root"
        ));

        let mut mismatched_vote_root_certificate = valid_block_certificate(hex96);
        mismatched_vote_root_certificate["votes"][0]["registry_root"] = json!(other_hex96);
        let mismatched_vote_root_block = success_response(
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
                        "certificate": mismatched_vote_root_certificate
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(&mismatched_vote_root_block, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "registry_root"
        ));

        let mut noncompact_certificate = valid_block_certificate(hex96);
        noncompact_certificate["votes"][0]["public_key_hex"] = json!(hex96);
        let noncompact_certificate_block = success_response(
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
                        "certificate": noncompact_certificate
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(&noncompact_certificate_block, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "public_key_hex"
        ));

        let bad_block_certificate_signature = success_response(
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
                        "certificate": {
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
                                    "signature_hex": "not-hex"
                                }
                            ]
                        }
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(&bad_block_certificate_signature, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "signature_hex"
        ));

        let bad_block_certificate_duplicate_validator = success_response(
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
                        "certificate": {
                            "validators": ["validator-0", "validator-0"],
                            "quorum": 2,
                            "registry_root": hex96,
                            "votes": [
                                {
                                    "vote_id": hex96,
                                    "validator": "validator-0",
                                    "accept": true,
                                    "algorithm_id": ML_DSA_65_ALGORITHM,
                                    "registry_root": hex96,
                                    "signature_hex": hex96
                                },
                                {
                                    "vote_id": other_hex96,
                                    "validator": "validator-0",
                                    "accept": true,
                                    "algorithm_id": ML_DSA_65_ALGORITHM,
                                    "registry_root": hex96,
                                    "signature_hex": other_hex96
                                }
                            ]
                        }
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(
                &bad_block_certificate_duplicate_validator,
                RpcResponseKind::Blocks
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "validators"
        ));

        let dirty_block_certificate_validator = success_response(
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
                        "certificate": {
                            "validators": ["validator-0 "],
                            "quorum": 1,
                            "registry_root": hex96,
                            "votes": [
                                {
                                    "vote_id": hex96,
                                    "validator": "validator-0 ",
                                    "accept": true,
                                    "algorithm_id": ML_DSA_65_ALGORITHM,
                                    "registry_root": hex96,
                                    "signature_hex": hex96
                                }
                            ]
                        }
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(
                &dirty_block_certificate_validator,
                RpcResponseKind::Blocks
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "validators"
        ));

        let bad_block_certificate_duplicate_vote_id = success_response(
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
                        "certificate": {
                            "validators": ["validator-0", "validator-1"],
                            "quorum": 2,
                            "registry_root": hex96,
                            "votes": [
                                {
                                    "vote_id": hex96,
                                    "validator": "validator-0",
                                    "accept": true,
                                    "algorithm_id": ML_DSA_65_ALGORITHM,
                                    "registry_root": hex96,
                                    "signature_hex": hex96
                                },
                                {
                                    "vote_id": hex96,
                                    "validator": "validator-1",
                                    "accept": true,
                                    "algorithm_id": ML_DSA_65_ALGORITHM,
                                    "registry_root": hex96,
                                    "signature_hex": other_hex96
                                }
                            ]
                        }
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(
                &bad_block_certificate_duplicate_vote_id,
                RpcResponseKind::Blocks
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "vote_id"
        ));

        let bad_block_certificate_duplicate_vote_validator = success_response(
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
                        "certificate": {
                            "validators": ["validator-0", "validator-1"],
                            "quorum": 2,
                            "registry_root": hex96,
                            "votes": [
                                {
                                    "vote_id": hex96,
                                    "validator": "validator-0",
                                    "accept": true,
                                    "algorithm_id": ML_DSA_65_ALGORITHM,
                                    "registry_root": hex96,
                                    "signature_hex": hex96
                                },
                                {
                                    "vote_id": other_hex96,
                                    "validator": "validator-0",
                                    "accept": true,
                                    "algorithm_id": ML_DSA_65_ALGORITHM,
                                    "registry_root": hex96,
                                    "signature_hex": other_hex96
                                }
                            ]
                        }
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(
                &bad_block_certificate_duplicate_vote_validator,
                RpcResponseKind::Blocks
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "validator"
        ));

        let bad_block_parent_hash = success_response(
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
                },
                {
                    "header": {
                        "height": 2,
                        "parent_hash": other_hex96,
                        "batch_kind": "transparent",
                        "batch_id": other_hex96,
                        "block_hash": other_hex96,
                        "state_root": hex96,
                        "receipt_count": 1,
                        "certificate_id": other_hex96,
                        "certificate": valid_block_certificate(hex96)
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        assert!(matches!(
            validate_response_kind(&bad_block_parent_hash, RpcResponseKind::Blocks),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "header.parent_hash"
        ));

        let bad_archive = success_response(
            "archive-1",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": "not-json"
                }
            ]),
            vec![],
        )
        .expect("archive response");
        assert!(matches!(
            validate_response_kind(&bad_archive, RpcResponseKind::BatchArchive),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "payload_json"
        ));

        let oversized_archive_payload = success_response(
            "archive-1",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": "x".repeat(MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES + 1)
                }
            ]),
            vec![],
        )
        .expect("archive response");
        assert!(matches!(
            validate_response_kind(&oversized_archive_payload, RpcResponseKind::BatchArchive),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "payload_json"
        ));

        let mismatched_archive_batch_id = success_response(
            "archive-1",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": valid_transparent_archive_payload(other_hex96)
                }
            ]),
            vec![],
        )
        .expect("archive response");
        assert!(matches!(
            validate_response_kind(&mismatched_archive_batch_id, RpcResponseKind::BatchArchive),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "payload_json.batch_id"
        ));

        let bad_archive_payload_hash = success_response(
            "archive-1",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": other_hex96,
                    "payload_json": valid_transparent_archive_payload(hex96)
                }
            ]),
            vec![],
        )
        .expect("archive response");
        assert!(matches!(
            validate_response_kind_with_context(
                &bad_archive_payload_hash,
                RpcResponseKind::BatchArchive,
                Some(&archive_context)
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "payload_hash"
        ));

        let malformed_archive_payload = success_response(
            "archive-1",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": format!("{{\"batch_id\":\"{hex96}\",\"transactions\":[{{}}]}}")
                }
            ]),
            vec![],
        )
        .expect("archive response");
        assert!(matches!(
            validate_response_kind(&malformed_archive_payload, RpcResponseKind::BatchArchive),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "unsigned"
        ));

        let leaked_archive_private_key = success_response(
            "archive-1",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": format!("{{\"batch_id\":\"{hex96}\",\"transactions\":[{{\"private_key_hex\":\"leaked\"}}]}}")
                }
            ]),
            vec![],
        )
        .expect("archive response");
        assert!(matches!(
            validate_response_kind(&leaked_archive_private_key, RpcResponseKind::BatchArchive),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "payload_json"
        ));

        let leaked_private_key = success_response(
            "account-1",
            &json!({
                "address": "pf1-test",
                "balance": 10,
                "sequence": 2,
                "private_key_hex": "leaked"
            }),
            vec![],
        )
        .expect("account response");
        assert!(matches!(
            validate_response_kind(&leaked_private_key, RpcResponseKind::Account),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "result"
        ));

        let leaked_orchard_note_seed = success_response(
            "account-1",
            &json!({
                "address": "pf1-test",
                "balance": 10,
                "sequence": 2,
                "rseed": "leaked"
            }),
            vec![],
        )
        .expect("account response");
        assert!(matches!(
            validate_response_kind(&leaked_orchard_note_seed, RpcResponseKind::Account),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "result"
        ));

        let bad_mempool_submit = success_response(
            "mempool-submit-1",
            &json!({
                "tx_id": "not-a-hash",
                "transfer": {
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
                }
            }),
            vec![],
        )
        .expect("mempool submit response");
        assert!(matches!(
            validate_response_kind(&bad_mempool_submit, RpcResponseKind::MempoolSubmitTransfer),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "tx_id"
        ));

        let bad_mempool_signature_hex = success_response(
            "mempool-submit-1",
            &json!({
                "tx_id": hex96,
                "transfer": {
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
                    "signature_hex": "not-hex"
                }
            }),
            vec![],
        )
        .expect("mempool submit response");
        assert!(matches!(
            validate_response_kind(
                &bad_mempool_signature_hex,
                RpcResponseKind::MempoolSubmitTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "signature_hex"
        ));

        let mut bad_mempool_chain_id_transfer = valid_signed_transfer(hex96);
        bad_mempool_chain_id_transfer["unsigned"]["chain_id"] = json!(" postfiat-local");
        let bad_mempool_chain_id = success_response(
            "mempool-submit-1",
            &json!({
                "tx_id": hex96,
                "transfer": bad_mempool_chain_id_transfer
            }),
            vec![],
        )
        .expect("mempool submit response");
        assert!(matches!(
            validate_response_kind(
                &bad_mempool_chain_id,
                RpcResponseKind::MempoolSubmitTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "chain_id"
        ));

        let mut bad_mempool_protocol_transfer = valid_signed_transfer(hex96);
        bad_mempool_protocol_transfer["unsigned"]["protocol_version"] = json!(0);
        let bad_mempool_protocol = success_response(
            "mempool-submit-1",
            &json!({
                "tx_id": hex96,
                "transfer": bad_mempool_protocol_transfer
            }),
            vec![],
        )
        .expect("mempool submit response");
        assert!(matches!(
            validate_response_kind(
                &bad_mempool_protocol,
                RpcResponseKind::MempoolSubmitTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "protocol_version"
        ));

        let bad_mempool_batch = success_response(
            "mempool-batch-1",
            &json!({
                "batch_id": hex96,
                "transactions": []
            }),
            vec![],
        )
        .expect("mempool batch response");
        assert!(matches!(
            validate_response_kind(&bad_mempool_batch, RpcResponseKind::MempoolBatch),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "transactions"
        ));

        let shield_wrong_kind = success_response(
            "shield-mint-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "shield_spend",
                        "note_id": hex96,
                        "to": "pfrecipient",
                        "amount": 10,
                        "memo": "wrong action"
                    }
                ]
            }),
            vec![],
        )
        .expect("shield mint response");
        assert!(matches!(
            validate_response_kind(&shield_wrong_kind, RpcResponseKind::ShieldBatchMint),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "kind"
        ));

        let zero_shield_mint_amount = success_response(
            "shield-mint-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "shield_mint",
                        "owner": "pfshieldowner",
                        "asset_id": "POSTFIAT",
                        "amount": 0,
                        "memo": "zero amount"
                    }
                ]
            }),
            vec![],
        )
        .expect("shield mint response");
        assert!(matches!(
            validate_response_kind(&zero_shield_mint_amount, RpcResponseKind::ShieldBatchMint),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "amount"
        ));

        let asset_orchard_swap_batch = success_response(
            "shield-swap-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "shielded_swap_v1",
                        "swap_json": valid_asset_orchard_swap_json()
                    }
                ]
            }),
            vec![],
        )
        .expect("shielded swap response");
        validate_response_kind(&asset_orchard_swap_batch, RpcResponseKind::ShieldBatchSwap)
            .expect("asset-orchard swap batch response");

        let bad_shield_note_id = success_response(
            "shield-scan-1",
            &json!([
                {
                    "note_id": "not-a-hash",
                    "commitment": hex96,
                    "position": 1,
                    "owner": "pfshieldowner",
                    "asset_id": "POSTFIAT",
                    "value": 42,
                    "rho": hex96,
                    "memo": "shielded note",
                    "created_by": hex96
                }
            ]),
            vec![],
        )
        .expect("shield scan response");
        assert!(matches!(
            validate_response_kind(&bad_shield_note_id, RpcResponseKind::ShieldScan),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "note_id"
        ));

        let leaked_shield_public_key = success_response(
            "shield-scan-1",
            &json!([
                {
                    "note_id": hex96,
                    "commitment": hex96,
                    "position": 1,
                    "owner": "pfshieldowner",
                    "asset_id": "POSTFIAT",
                    "value": 42,
                    "rho": hex96,
                    "memo": "shielded note",
                    "created_by": hex96,
                    "public_key_hex": "leaked"
                }
            ]),
            vec![],
        )
        .expect("shield scan response");
        assert!(matches!(
            validate_response_kind(&leaked_shield_public_key, RpcResponseKind::ShieldScan),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "result"
        ));

        let bridge_wrong_pause = success_response(
            "bridge-pause-1",
            &json!({
                "batch_id": hex96,
                "actions": [{"kind": "bridge_pause", "domain_id": "local-sim", "paused": false}]
            }),
            vec![],
        )
        .expect("bridge pause response");
        assert!(matches!(
            validate_response_kind(&bridge_wrong_pause, RpcResponseKind::BridgeBatchPause),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "paused"
        ));

        let zero_bridge_domain_cap = success_response(
            "bridge-domain-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "bridge_domain",
                        "domain_id": "local-sim",
                        "name": "Local Simulation",
                        "source_chain": "external",
                        "target_chain": "postfiat-local",
                        "bridge_id": "local-sim",
                        "door_account": "door:local-sim",
                        "inbound_cap": 0,
                        "outbound_cap": 60
                    }
                ]
            }),
            vec![],
        )
        .expect("bridge domain response");
        assert!(matches!(
            validate_response_kind(&zero_bridge_domain_cap, RpcResponseKind::BridgeBatchDomain),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "inbound_cap"
        ));

        let bridge_dirty_domain_name = success_response(
            "bridge-domain-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "bridge_domain",
                        "domain_id": "local-sim",
                        "name": " Local Simulation",
                        "source_chain": "external",
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
        assert!(matches!(
            validate_response_kind(&bridge_dirty_domain_name, RpcResponseKind::BridgeBatchDomain),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "name"
        ));

        let bridge_bad_direction = success_response(
            "bridge-transfer-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "bridge_transfer",
                        "domain_id": "local-sim",
                        "direction": "sideways",
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
        assert!(matches!(
            validate_response_kind(&bridge_bad_direction, RpcResponseKind::BridgeBatchTransfer),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "direction"
        ));

        let bridge_dirty_witness_id = success_response(
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
                        "witness_id": "witness-2 ",
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
        assert!(matches!(
            validate_response_kind(&bridge_dirty_witness_id, RpcResponseKind::BridgeBatchTransfer),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "witness_id"
        ));

        let bridge_bad_attestation_id = success_response(
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
                            "attestation_id": "not-a-hash",
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
        assert!(matches!(
            validate_response_kind(
                &bridge_bad_attestation_id,
                RpcResponseKind::BridgeBatchTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "attestation_id"
        ));

        let bridge_bad_attestation_chain_id = success_response(
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
                            "chain_id": " postfiat-local",
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
        assert!(matches!(
            validate_response_kind(
                &bridge_bad_attestation_chain_id,
                RpcResponseKind::BridgeBatchTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "chain_id"
        ));

        let bridge_bad_attestation_protocol_version = success_response(
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
                            "protocol_version": 4294967296_u64,
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
        assert!(matches!(
            validate_response_kind(
                &bridge_bad_attestation_protocol_version,
                RpcResponseKind::BridgeBatchTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "protocol_version"
        ));

        let bridge_bad_attestation_signer = success_response(
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
                            "signer": "validator-0\n",
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
        assert!(matches!(
            validate_response_kind(
                &bridge_bad_attestation_signer,
                RpcResponseKind::BridgeBatchTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "signer"
        ));

        let bridge_bad_public_key_hex = success_response(
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
                            "public_key_hex": "not-hex",
                            "signature_hex": hex96
                        }
                    }
                ]
            }),
            vec![],
        )
        .expect("bridge transfer response");
        assert!(matches!(
            validate_response_kind(
                &bridge_bad_public_key_hex,
                RpcResponseKind::BridgeBatchTransfer
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "public_key_hex"
        ));

        let bridge_bad_transfer_id = success_response(
            "bridge-status-1",
            &json!({
                "domains": [],
                "transfers": [
                    {
                        "transfer_id": "not-a-hash",
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
                "replay_cache": []
            }),
            vec![],
        )
        .expect("bridge status response");
        assert!(matches!(
            validate_response_kind(&bridge_bad_transfer_id, RpcResponseKind::BridgeStatus),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "transfer_id"
        ));

        let bridge_dirty_replay_cache = success_response(
            "bridge-status-1",
            &json!({
                "domains": [],
                "transfers": [],
                "replay_cache": ["local-sim:1:witness-1\n"]
            }),
            vec![],
        )
        .expect("bridge status response");
        assert!(matches!(
            validate_response_kind(&bridge_dirty_replay_cache, RpcResponseKind::BridgeStatus),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "replay_cache"
        ));

        let leaked_bridge_private_key = success_response(
            "bridge-status-1",
            &json!({
                "domains": [],
                "transfers": [{"private_key_hex": "leaked"}],
                "replay_cache": []
            }),
            vec![],
        )
        .expect("bridge status response");
        assert!(matches!(
            validate_response_kind(&leaked_bridge_private_key, RpcResponseKind::BridgeStatus),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "result"
        ));

        let hex64 = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
        let navcoin_packet_mismatch = success_response(
            "navcoin-bridge-packet-1",
            &json!({
                "schema": "postfiat-pftl-uniswap-packet-status-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "packet_hash": hex96,
                "packet": {
                    "packet_hash": "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
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
        assert!(matches!(
            validate_response_kind(
                &navcoin_packet_mismatch,
                RpcResponseKind::NavcoinBridgePacket
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "packet.packet_hash"
        ));

        let navcoin_preflight_not_ready = success_response(
            "navcoin-bridge-packet-preflight-1",
            &json!({
                "schema": "postfiat-navcoin-bridge-packet-preflight-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "launch_config_digest": hex96,
                "packet_digest": hex96,
                "ledger_hash": hex96,
                "packet_file": "devnet/local/pftl-uniswap/packet.json",
                "status": "needs_attention"
            }),
            vec![],
        )
        .expect("NAVCoin bridge packet preflight response");
        assert!(matches!(
            validate_response_kind(
                &navcoin_preflight_not_ready,
                RpcResponseKind::NavcoinBridgePacketPreflight
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "status"
        ));

        let navcoin_claim_class_mismatch = success_response(
            "navcoin-bridge-claims-1",
            &json!({
                "schema": "postfiat-pftl-uniswap-claims-status-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "ledger_hash": hex96,
                "limit": 1,
                "truncated": false,
                "outstanding_bridge_claims_atoms": 100,
                "pending_return_import_claims_atoms": 0,
                "export_claim_count": 1,
                "return_claim_count": 0,
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
                        "claim_class": "source_refunded"
                    }
                ],
                "returns": []
            }),
            vec![],
        )
        .expect("NAVCoin bridge claims response");
        assert!(matches!(
            validate_response_kind(
                &navcoin_claim_class_mismatch,
                RpcResponseKind::NavcoinBridgeClaims
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "claim_class"
        ));

        let navcoin_supply_bad_sum = success_response(
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
                "live_supply_sum_atoms": 299,
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
        assert!(matches!(
            validate_response_kind(
                &navcoin_supply_bad_sum,
                RpcResponseKind::NavcoinBridgeSupplyStatus
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "live_supply_sum_atoms"
        ));

        let navcoin_replay_count_mismatch = success_response(
            "navcoin-bridge-replay-1",
            &json!({
                "schema": "postfiat-navcoin-bridge-receipt-replay-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "initial_ledger_hash": hex96,
                "final_ledger_hash": hex96,
                "receipt_root": hex96,
                "receipt_count": 2,
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
        assert!(matches!(
            validate_response_kind(
                &navcoin_replay_count_mismatch,
                RpcResponseKind::NavcoinBridgeReceiptReplay
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "replay.receipt_count"
        ));
    }

    #[test]
    fn health_response_validation_rejects_bad_schema_and_key_leaks() {
        let bad_status = success_response(
            "status-1",
            &json!({
                "chain_id": "postfiat-local",
                "genesis_hash": "not-a-genesis-hash",
                "protocol_version": 1,
                "validator_count": 4,
                "node_id": "validator-0",
                "status": "running",
                "last_run_unix": 1,
                "state_root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "block_height": 0,
                "block_tip_hash": "genesis",
                "mempool_pending": 0
            }),
            vec![],
        )
        .expect("status response");
        assert!(matches!(
            validate_health_response(&bad_status, RpcResponseKind::Status),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "genesis_hash"
        ));

        let dirty_status_node = success_response(
            "status-1",
            &json!({
                "chain_id": "postfiat-local",
                "genesis_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "protocol_version": 1,
                "validator_count": 4,
                "node_id": "validator-0\n",
                "status": "running",
                "last_run_unix": 1,
                "state_root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "block_height": 0,
                "block_tip_hash": "genesis",
                "mempool_pending": 0
            }),
            vec![],
        )
        .expect("status response");
        assert!(matches!(
            validate_health_response(&dirty_status_node, RpcResponseKind::Status),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "node_id"
        ));

        let metrics = success_response(
            "metrics-1",
            &json!({
                "schema": "wrong-schema",
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
                "mempool": {"pending": 0},
                "storage": {"replicated_state_file_count": 12},
                "shielded": {"note_count": 0, "nullifier_count": 0, "turnstile_event_count": 0},
                "bridge": {"domain_count": 0, "transfer_count": 0, "replay_cache_count": 0}
            }),
            vec![],
        )
        .expect("metrics response");
        assert!(matches!(
            validate_health_response(&metrics, RpcResponseKind::Metrics),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "schema"
        ));

        let dirty_metrics_node = success_response(
            "metrics-1",
            &json!({
                "schema": METRICS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "protocol_version": 1,
                "node_id": " validator-0"
            }),
            vec![],
        )
        .expect("metrics response");
        assert!(matches!(
            validate_health_response(&dirty_metrics_node, RpcResponseKind::Metrics),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "node_id"
        ));

        let bad_metrics = success_response(
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
                    "block_tip_hash": "genesis",
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
                "mempool": {"pending": 0},
                "storage": {"replicated_state_file_count": 12},
                "shielded": {"note_count": 0, "nullifier_count": 0, "turnstile_event_count": 0},
                "bridge": {"domain_count": 0, "transfer_count": 0, "replay_cache_count": 0}
            }),
            vec![],
        )
        .expect("metrics response");
        assert!(matches!(
            validate_health_response(&bad_metrics, RpcResponseKind::Metrics),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "ordering.block_tip_hash"
        ));

        let bad_state = success_response(
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
                    "tip_hash": "genesis",
                    "state_root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                }
            }),
            vec![],
        )
        .expect("state response");
        assert!(matches!(
            validate_health_response(&bad_state, RpcResponseKind::VerifyState),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "block_log.tip_hash"
        ));

        let dirty_state_chain = success_response(
            "state-1",
            &json!({
                "schema": STATE_VERIFICATION_SCHEMA,
                "verified": true,
                "chain_id": "postfiat-local ",
                "genesis_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "protocol_version": 1
            }),
            vec![],
        )
        .expect("state response");
        assert!(matches!(
            validate_health_response(&dirty_state_chain, RpcResponseKind::VerifyState),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "chain_id"
        ));

        let bad_keys = success_response(
            "keys-1",
            &json!({
                "schema": LOCAL_KEY_VALIDATION_SCHEMA,
                "node_id": "validator-0",
                "faucet_address": "pffaucet",
                "required_validator_count": 4,
                "validator_key_count": 4,
                "faucet_key_valid": true,
                "faucet_key_permissions_valid": false,
                "validator_keys_valid": true,
                "validator_key_permissions_valid": true
            }),
            vec![],
        )
        .expect("keys response");
        assert!(matches!(
            validate_health_response(
                &bad_keys,
                RpcResponseKind::ValidateLocalKeys {
                    validators: Some(4)
                }
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "faucet_key_permissions_valid"
        ));

        let dirty_keys = success_response(
            "keys-1",
            &json!({
                "schema": LOCAL_KEY_VALIDATION_SCHEMA,
                "node_id": "validator-0",
                "faucet_address": " pffaucet",
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
        assert!(matches!(
            validate_health_response(
                &dirty_keys,
                RpcResponseKind::ValidateLocalKeys {
                    validators: Some(4)
                }
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "faucet_address"
        ));

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
                "validator_key_permissions_valid": true,
                "validators": [{"public_key_hex": "leaked"}]
            }),
            vec![],
        )
        .expect("keys response");
        assert!(matches!(
            validate_health_response(
                &keys,
                RpcResponseKind::ValidateLocalKeys {
                    validators: Some(4)
                }
            ),
            Err(RpcResponseValidationError::InvalidResult { field, .. }) if field == "result"
        ));
    }

    #[test]
    fn response_protocol_validation_rejects_bad_envelopes() {
        let mut bad_version =
            success_response("status-1", &json!({"ok": true}), vec![]).expect("response");
        bad_version.version = "postfiat-local-rpc-v0".to_string();
        assert_eq!(
            bad_version.validate_protocol().expect_err("bad version"),
            RpcProtocolError::UnsupportedVersion {
                found: "postfiat-local-rpc-v0".to_string(),
            }
        );

        let mut empty_id = success_response(" ", &json!({"ok": true}), vec![]).expect("response");
        assert_eq!(
            empty_id.validate_protocol().expect_err("empty id"),
            RpcProtocolError::EmptyId,
        );

        empty_id.id = "missing-result".to_string();
        empty_id.result = None;
        assert_eq!(
            empty_id
                .validate_protocol()
                .expect_err("missing success result"),
            RpcProtocolError::ResponseMissingResult,
        );

        let mut success_with_error =
            success_response("bad-success", &json!({"ok": true}), vec![]).expect("response");
        success_with_error.error = Some(RpcError {
            code: "rpc_error".to_string(),
            message: "unexpected".to_string(),
        });
        assert_eq!(
            success_with_error
                .validate_protocol()
                .expect_err("success with error"),
            RpcProtocolError::ResponseUnexpectedError,
        );

        let mut error_with_result = error_response("bad-error", "rpc_error", "failed", vec![]);
        error_with_result.result = Some(json!({"ok": false}));
        assert_eq!(
            error_with_result
                .validate_protocol()
                .expect_err("error with result"),
            RpcProtocolError::ResponseUnexpectedResult,
        );

        let mut missing_error = error_response("missing-error", "rpc_error", "failed", vec![]);
        missing_error.error = None;
        assert_eq!(
            missing_error
                .validate_protocol()
                .expect_err("missing error"),
            RpcProtocolError::ResponseMissingError,
        );

        let key_marker_error = error_response(
            "key-marker-error",
            "rpc_error",
            "private_key_hex leaked",
            vec![],
        );
        assert_eq!(
            key_marker_error
                .validate_protocol()
                .expect_err("key marker error"),
            RpcProtocolError::ResponseErrorKeyMaterial { field: "message" },
        );
        assert!(matches!(
            validate_response(&key_marker_error, Some("key-marker-error"), false),
            Err(RpcResponseValidationError::Protocol(
                RpcProtocolError::ResponseErrorKeyMaterial { field: "message" }
            ))
        ));

        let mut empty_event_field = success_response(
            "empty-event",
            &json!({"ok": true}),
            vec![RpcEvent::new("status", "node-0", "status queried")],
        )
        .expect("response");
        empty_event_field.events[0].message = " ".to_string();
        assert_eq!(
            empty_event_field
                .validate_protocol()
                .expect_err("empty event message"),
            RpcProtocolError::ResponseEmptyEventField {
                index: 0,
                field: "message",
            },
        );
        assert!(matches!(
            validate_response(&empty_event_field, Some("empty-event"), true),
            Err(RpcResponseValidationError::Protocol(
                RpcProtocolError::ResponseEmptyEventField {
                    index: 0,
                    field: "message"
                }
            ))
        ));

        let key_marker_event = success_response(
            "key-marker-event",
            &json!({"ok": true}),
            vec![RpcEvent::new("status", "private_key_hex", "status queried")],
        )
        .expect("response");
        assert_eq!(
            key_marker_event
                .validate_protocol()
                .expect_err("key marker event"),
            RpcProtocolError::ResponseEventKeyMaterial {
                index: 0,
                field: "subject",
            },
        );
    }
