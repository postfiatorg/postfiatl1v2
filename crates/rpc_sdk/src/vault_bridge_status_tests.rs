mod vault_bridge_status {
    use super::*;
    use serde_json::{Value, json};

    fn asset_id() -> String {
        "ab".repeat(48)
    }

    fn with(mut value: Value, pointer: &str, replacement: Value) -> Value {
        *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
        value
    }

    fn buckets() -> Value {
        json!([
                {
                    "bucket_id": "bucket-active",
                    "source_series_id": "series-1",
                    "source_domain": "evm:1:0xvault:0xtoken",
                    "policy_hash": "11".repeat(48),
                    "gross_receipt_atoms": 500,
                    "counted_value_atoms": 500,
                    "outstanding_vault_bridge_atoms": 100,
                    "nav_subscription_allocations_atoms": 50,
                    "redemption_queue_atoms": 25,
                    "other_allocations_atoms": 25,
                    "unallocated_counted_capacity_atoms": 300,
                    "impairment_factor_bps": 10_000,
                    "redeemable_claim_atoms": 200,
                    "redeemable_at_par": true,
                    "status": "active",
                    "last_packet_epoch": 7,
                    "last_updated_height": 70
                },
                {
                    "bucket_id": "bucket-impaired",
                    "source_series_id": "series-2",
                    "source_domain": "evm:1:0xvault:0xtoken",
                    "policy_hash": "22".repeat(48),
                    "gross_receipt_atoms": 80,
                    "counted_value_atoms": 80,
                    "outstanding_vault_bridge_atoms": 40,
                    "nav_subscription_allocations_atoms": 0,
                    "redemption_queue_atoms": 10,
                    "other_allocations_atoms": 0,
                    "unallocated_counted_capacity_atoms": 0,
                    "impairment_factor_bps": 9_000,
                    "redeemable_claim_atoms": 45,
                    "redeemable_at_par": false,
                    "status": "impaired",
                    "last_packet_epoch": 6,
                    "last_updated_height": 60
                }
        ])
    }

    fn receipts() -> Value {
        json!([
                {
                    "receipt_id": "receipt-1",
                    "bucket_id": "bucket-active",
                    "source_domain": "evm:1:0xvault:0xtoken",
                    "source_asset": "USDC",
                    "claim_type": "bridge_deposit",
                    "amount_atoms": 500,
                    "haircut_bps": 0,
                    "counted_value_atoms": 500,
                    "allocated_value_atoms": 200,
                    "unallocated_value_atoms": 300,
                    "status": "counted",
                    "created_at_height": 10,
                    "counted_at_height": 11,
                    "expires_at_height": 1000,
                    "source_tx_or_attestation": "0xdeposit",
                    "finality_ref": "finality-1",
                    "vault_id": "vault-1",
                    "bridge_deposit_evidence_root": "33".repeat(48)
                },
                {
                    "receipt_id": "receipt-2",
                    "bucket_id": "bucket-impaired",
                    "source_domain": "evm:1:0xvault:0xtoken",
                    "source_asset": "USDC",
                    "claim_type": "bridge_deposit",
                    "amount_atoms": 80,
                    "haircut_bps": 0,
                    "counted_value_atoms": 80,
                    "allocated_value_atoms": 50,
                    "unallocated_value_atoms": 0,
                    "status": "counted",
                    "created_at_height": 12,
                    "counted_at_height": 13,
                    "expires_at_height": 1000,
                    "source_tx_or_attestation": "",
                    "finality_ref": "",
                    "vault_id": "vault-1"
                }
        ])
    }

    fn bridge_deposits() -> Value {
        json!([
                {
                    "evidence_root": "33".repeat(48),
                    "policy_hash": "11".repeat(48),
                    "source_proof_kind": "receipt-proof",
                    "source_proof_hash": "44".repeat(48),
                    "source_public_values_hash": "55".repeat(48),
                    "source_chain_id": 1,
                    "vault_address": "0x1111111111111111111111111111111111111111",
                    "token_address": "0x2222222222222222222222222222222222222222",
                    "depositor": "0x3333333333333333333333333333333333333333",
                    "pftl_recipient": "pfrecipient",
                    "amount_atoms": 500,
                    "deposit_id": "deposit-1",
                    "block_hash": "66".repeat(32),
                    "tx_hash": "77".repeat(32),
                    "log_index": 3,
                    "proposer": "pfproposer",
                    "status": "finalized",
                    "submitted_at_height": 9,
                    "finalized_at_height": 10,
                    "expires_at_height": 1000,
                    "challenger": "",
                    "challenge_hash": "",
                    "challenge_bond": 0,
                    "pass_attestation_count": 1,
                    "fail_attestation_count": 1,
                    "attestations": [
                        {
                            "attestor": "pfattestor1",
                            "pass": true,
                            "observation_root": "88".repeat(48),
                            "attested_at_height": 9
                        },
                        {
                            "attestor": "pfattestor2",
                            "pass": false,
                            "observation_root": "99".repeat(48),
                            "attested_at_height": 9
                        }
                    ]
                }
        ])
    }

    fn allocations() -> Value {
        json!([
                {
                    "allocation_id": "allocation-1",
                    "receipt_id": "receipt-1",
                    "bucket_id": "bucket-active",
                    "amount_atoms": 200,
                    "released_atoms": 50,
                    "remaining_atoms": 150,
                    "purpose": "nav_subscription",
                    "consumer_id": "consumer-1",
                    "created_at_height": 20,
                    "retired_at_height": 0
                }
        ])
    }

    fn redemptions() -> Value {
        json!([
                {
                    "redemption_id": "redemption-1",
                    "owner": "pfowner",
                    "owner_sequence": 4,
                    "issuer": "pfissuer",
                    "bucket_id": "bucket-active",
                    "amount_atoms": 10,
                    "epoch": 7,
                    "reserve_packet_hash": "cd".repeat(48),
                    "destination_ref": "evm:0x4444444444444444444444444444444444444444",
                    "settled_atoms": 0,
                    "state": "queued",
                    "created_at_height": 30,
                    "settlement_receipt_hash": "",
                    "burn_tx_id": "",
                    "withdrawal_recipient": "",
                    "withdrawal_evidence_root": "",
                    "withdrawal_packet_hash": "",
                    "withdrawal_packet_evm_digest": ""
                }
        ])
    }

    /// A report shaped like the node's `VaultBridgeStatusReport`: one active
    /// bucket at par and one impaired bucket, a receipt in each, one deposit
    /// with a pass and a fail attestation, one allocation, one redemption.
    fn fixture() -> Value {
        json!({
            "schema": "postfiat-vault-bridge-status-v1",
            "asset_id": asset_id(),
            "issuer": "pfissuer",
            "proof_profile": "nav-reserve-v1",
            "valuation_unit": "USD",
            "finalized_epoch": 7,
            "nav_per_unit": 1_000_000,
            "circulating_supply": 1000,
            "finalized_reserve_packet_hash": "cd".repeat(48),
            "issued_supply_atoms": 1000,
            "transparent_supply_atoms": 900,
            "orchard_supply_atoms": 100,
            "counted_value_atoms": 580,
            "healthy_allocated_atoms": 200,
            "impaired_allocated_atoms": 50,
            "source_series_enforced": false,
            "display_family_classification": "legacy_pooled",
            "unallocated_counted_capacity_atoms": 300,
            "source_root": "ef".repeat(48),
            "bucket_count": 2,
            "receipt_count": 2,
            "bridge_deposit_count": 1,
            "allocation_count": 1,
            "redemption_count": 1,
            "buckets": buckets(),
            "receipts": receipts(),
            "bridge_deposits": bridge_deposits(),
            "allocations": allocations(),
            "redemptions": redemptions(),
            "disclosure": "vault bridge asset is source-bound and bridge-backed on PFTL. Counted vault bridge asset receipts must reference a ERC20BridgeVault deposit event evidence root; withdrawals are PFTL burn packets claimed from the source vault after challenge/finality. No pooled or automatic par redemption is implied."
        })
    }

    fn validate(report: &Value) -> Result<(), RpcResponseValidationError> {
        validate_response_kind(
            &success_response("vault-bridge-status-1", report, vec![])
                .expect("vault bridge status response"),
            RpcResponseKind::VaultBridgeStatus,
        )
    }

    fn rejected_field(report: Value) -> String {
        match validate(&report) {
            Err(RpcResponseValidationError::InvalidResult { field, .. }) => field,
            other => panic!("expected an InvalidResult rejection, got {other:?}"),
        }
    }

    #[test]
    fn builds_and_validates_vault_bridge_status_request() {
        let request = vault_bridge_status_request(
            "vault-bridge-status-1",
            VaultBridgeStatusParams {
                asset_id: asset_id(),
            },
        );
        assert_eq!(request.method, METHOD_VAULT_BRIDGE_STATUS);
        assert_eq!(request.params["asset_id"], json!(asset_id()));
        validate_request(
            &request,
            Some("vault-bridge-status-1"),
            Some(RpcRequestKind::VaultBridgeStatus),
        )
        .expect("vault bridge status request");
    }

    #[test]
    fn rejects_vault_bridge_status_request_params() {
        let uppercase = RpcRequest::empty("vault-bridge-status-2", METHOD_VAULT_BRIDGE_STATUS)
            .with_param_value("asset_id", string_value("AB".repeat(48)));
        assert!(validate_request(&uppercase, None, Some(RpcRequestKind::VaultBridgeStatus)).is_err());

        let short = RpcRequest::empty("vault-bridge-status-3", METHOD_VAULT_BRIDGE_STATUS)
            .with_param_value("asset_id", string_value("ab".repeat(47)));
        assert!(validate_request(&short, None, Some(RpcRequestKind::VaultBridgeStatus)).is_err());

        let missing = RpcRequest::empty("vault-bridge-status-4", METHOD_VAULT_BRIDGE_STATUS);
        assert!(validate_request(&missing, None, Some(RpcRequestKind::VaultBridgeStatus)).is_err());

        let extra = vault_bridge_status_request(
            "vault-bridge-status-5",
            VaultBridgeStatusParams {
                asset_id: asset_id(),
            },
        )
        .with_param_value("epoch", string_value("7"));
        assert!(validate_request(&extra, None, Some(RpcRequestKind::VaultBridgeStatus)).is_err());
    }

    #[test]
    fn accepts_vault_bridge_status_report() {
        validate(&fixture()).expect("valid vault bridge status report");
    }

    #[test]
    fn accepts_source_series_classification_when_enforced() {
        let report = with(fixture(), "/source_series_enforced", json!(true));
        let report = with(
            report,
            "/display_family_classification",
            json!("mixed_legacy_pooled_and_source_series"),
        );
        validate(&report).expect("source-series classification");
    }

    #[test]
    fn rejects_wrong_schema() {
        let report = with(fixture(), "/schema", json!("postfiat-vault-bridge-status-v2"));
        assert_eq!(rejected_field(report), "schema");
    }

    #[test]
    fn rejects_uppercase_asset_id() {
        let report = with(fixture(), "/asset_id", json!("AB".repeat(48)));
        assert_eq!(rejected_field(report), "asset_id");
    }

    #[test]
    fn rejects_supply_that_does_not_split_into_transparent_and_orchard() {
        let report = with(fixture(), "/orchard_supply_atoms", json!(101));
        assert_eq!(rejected_field(report), "issued_supply_atoms");
    }

    #[test]
    fn rejects_classification_that_disagrees_with_enforcement_flag() {
        let report = with(fixture(), "/source_series_enforced", json!(true));
        assert_eq!(rejected_field(report), "display_family_classification");
    }

    #[test]
    fn rejects_healthy_and_impaired_sums_that_disagree_with_bucket_rows() {
        let report = with(fixture(), "/healthy_allocated_atoms", json!(199));
        assert_eq!(rejected_field(report), "healthy_allocated_atoms");
        let report = with(fixture(), "/impaired_allocated_atoms", json!(51));
        assert_eq!(rejected_field(report), "impaired_allocated_atoms");
    }

    #[test]
    fn rejects_bucket_unallocated_capacity_mismatch() {
        let report = with(
            fixture(),
            "/buckets/0/unallocated_counted_capacity_atoms",
            json!(299),
        );
        assert_eq!(
            rejected_field(report),
            "buckets[0].unallocated_counted_capacity_atoms"
        );
        let report = with(
            fixture(),
            "/buckets/1/unallocated_counted_capacity_atoms",
            json!(30),
        );
        assert_eq!(
            rejected_field(report),
            "buckets[1].unallocated_counted_capacity_atoms"
        );
    }

    #[test]
    fn rejects_bucket_allocations_above_counted_value() {
        let report = with(fixture(), "/buckets/0/other_allocations_atoms", json!(400));
        assert_eq!(
            rejected_field(report),
            "buckets[0].unallocated_counted_capacity_atoms"
        );
    }

    #[test]
    fn rejects_redeemable_at_par_flag_that_disagrees_with_status_and_factor() {
        let report = with(fixture(), "/buckets/1/redeemable_at_par", json!(true));
        assert_eq!(rejected_field(report), "buckets[1].redeemable_at_par");
        let report = with(fixture(), "/buckets/0/impairment_factor_bps", json!(9_999));
        assert_eq!(rejected_field(report), "buckets[0].redeemable_at_par");
    }

    #[test]
    fn rejects_redeemable_claim_that_ignores_the_impairment_factor() {
        let report = with(fixture(), "/buckets/1/redeemable_claim_atoms", json!(50));
        assert_eq!(rejected_field(report), "buckets[1].redeemable_claim_atoms");
    }

    #[test]
    fn rejects_receipt_unallocated_value_mismatch() {
        let report = with(fixture(), "/receipts/0/unallocated_value_atoms", json!(0));
        assert_eq!(rejected_field(report), "receipts[0].unallocated_value_atoms");
        let report = with(fixture(), "/receipts/1/unallocated_value_atoms", json!(30));
        assert_eq!(rejected_field(report), "receipts[1].unallocated_value_atoms");
        let report = with(fixture(), "/receipts/0/allocated_value_atoms", json!(501));
        assert_eq!(rejected_field(report), "receipts[0].allocated_value_atoms");
    }

    #[test]
    fn rejects_attestation_tallies_that_disagree_with_rows() {
        let report = with(fixture(), "/bridge_deposits/0/pass_attestation_count", json!(2));
        assert_eq!(
            rejected_field(report),
            "bridge_deposits[0].pass_attestation_count"
        );
        let report = with(fixture(), "/bridge_deposits/0/fail_attestation_count", json!(0));
        assert_eq!(
            rejected_field(report),
            "bridge_deposits[0].fail_attestation_count"
        );
    }

    #[test]
    fn rejects_allocation_remaining_mismatch() {
        let report = with(fixture(), "/allocations/0/remaining_atoms", json!(151));
        assert_eq!(rejected_field(report), "allocations[0].remaining_atoms");
        let report = with(fixture(), "/allocations/0/released_atoms", json!(201));
        assert_eq!(rejected_field(report), "allocations[0].remaining_atoms");
    }

    #[test]
    fn rejects_row_counts_that_disagree_with_arrays() {
        for (pointer, field) in [
            ("/bucket_count", "bucket_count"),
            ("/receipt_count", "receipt_count"),
            ("/bridge_deposit_count", "bridge_deposit_count"),
            ("/allocation_count", "allocation_count"),
            ("/redemption_count", "redemption_count"),
        ] {
            let report = with(fixture(), pointer, json!(9));
            assert_eq!(rejected_field(report), field);
        }
    }

    #[test]
    fn rejects_empty_identifier_fields_in_rows() {
        let report = with(fixture(), "/buckets/0/bucket_id", json!(""));
        assert_eq!(rejected_field(report), "bucket_id");
        let report = with(fixture(), "/redemptions/0/state", json!(""));
        assert_eq!(rejected_field(report), "state");
        let report = with(fixture(), "/bridge_deposits/0/attestations/1/attestor", json!(""));
        assert_eq!(rejected_field(report), "attestor");
    }

    #[test]
    fn rejects_altered_disclosure() {
        let report = with(fixture(), "/disclosure", json!("par redemption is implied"));
        assert_eq!(rejected_field(report), "disclosure");
    }
}
