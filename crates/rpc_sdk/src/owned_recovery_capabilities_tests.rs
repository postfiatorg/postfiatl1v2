mod owned_recovery_capabilities {
    use super::*;
    use postfiat_types::{
        FastPayRecoveryCapabilitiesV1, FastPayRecoveryPolicyV1, OwnedCertificateDomain,
        FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1, FASTPAY_RECOVERY_POLICY_SCHEMA_V1,
        MAX_FASTPAY_RECOVERY_BLOCKS, MAX_FASTPAY_VALIDITY_BLOCKS,
        OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
    };
    use serde_json::{Value, json};

    fn with(mut value: Value, pointer: &str, replacement: Value) -> Value {
        *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
        value
    }

    /// A report built from the real types, so it is exactly what the node
    /// serializes after `FastPayRecoveryCapabilitiesV1::validate` passes.
    fn capabilities() -> FastPayRecoveryCapabilitiesV1 {
        FastPayRecoveryCapabilitiesV1 {
            schema: FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1.to_string(),
            domain: OwnedCertificateDomain {
                schema: OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
                chain_id: "rpc-sdk-tests".to_string(),
                genesis_hash: "11".repeat(48),
                protocol_version: 3,
                registry_id: "22".repeat(48),
            },
            committee_epoch: 7,
            current_height: 120,
            validator_count: 4,
            quorum: 3,
            policy: FastPayRecoveryPolicyV1 {
                schema: FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
                activation_height: 90,
                max_validity_blocks: 20,
                max_recovery_blocks: 20,
            },
        }
    }

    fn report() -> Value {
        let report = capabilities();
        report.validate().expect("fixture passes the type's own validate");
        serde_json::to_value(report).expect("capabilities json")
    }

    fn validate(report: &Value) -> Result<(), RpcResponseValidationError> {
        validate_response_kind(
            &success_response("owned-recovery-capabilities-1", report, vec![])
                .expect("owned recovery capabilities response"),
            RpcResponseKind::OwnedRecoveryCapabilities,
        )
    }

    fn rejected_field(report: Value) -> String {
        match validate(&report) {
            Err(RpcResponseValidationError::InvalidResult { field, .. }) => field,
            other => panic!("expected an InvalidResult rejection, got {other:?}"),
        }
    }

    #[test]
    fn builds_and_validates_owned_recovery_capabilities_request() {
        let request = owned_recovery_capabilities_request("owned-recovery-capabilities-1");
        assert_eq!(request.method, METHOD_OWNED_RECOVERY_CAPABILITIES);
        validate_request(
            &request,
            Some("owned-recovery-capabilities-1"),
            Some(RpcRequestKind::OwnedRecoveryCapabilities),
        )
        .expect("owned recovery capabilities request");
    }

    #[test]
    fn rejects_any_request_params() {
        let request = owned_recovery_capabilities_request("owned-recovery-capabilities-2")
            .with_param_value("lock_id", string_value("ab".repeat(48)));
        assert!(
            validate_request(
                &request,
                None,
                Some(RpcRequestKind::OwnedRecoveryCapabilities)
            )
            .is_err()
        );
    }

    #[test]
    fn accepts_capabilities_report() {
        validate(&report()).expect("valid capabilities report");
    }

    #[test]
    fn rejects_wrong_schema() {
        let value = with(report(), "/schema", json!("postfiat-fastpay-recovery-capabilities-v2"));
        assert_eq!(rejected_field(value), "schema");
    }

    #[test]
    fn rejects_domain_problems_by_field() {
        let value = with(report(), "/domain/schema", json!("postfiat-owned-certificate-domain-v2"));
        assert_eq!(rejected_field(value), "domain.schema");
        let value = with(report(), "/domain/chain_id", json!("   "));
        assert_eq!(rejected_field(value), "domain.chain_id");
        let value = with(report(), "/domain/genesis_hash", json!("11".repeat(47)));
        assert_eq!(rejected_field(value), "domain.genesis_hash");
        let value = with(report(), "/domain/genesis_hash", json!("AA".repeat(48)));
        assert_eq!(rejected_field(value), "domain.genesis_hash");
        let value = with(report(), "/domain/protocol_version", json!(0));
        assert_eq!(rejected_field(value), "domain.protocol_version");
        let value = with(report(), "/domain/registry_id", json!(""));
        assert_eq!(rejected_field(value), "domain.registry_id");
    }

    #[test]
    fn rejects_zero_committee_epoch() {
        let value = with(report(), "/committee_epoch", json!(0));
        assert_eq!(rejected_field(value), "committee_epoch");
    }

    #[test]
    fn rejects_non_integer_current_height() {
        let value = with(report(), "/current_height", json!(-1));
        assert_eq!(rejected_field(value), "current_height");
    }

    #[test]
    fn rejects_committee_sizes_the_node_forbids() {
        let value = with(report(), "/validator_count", json!(0));
        assert_eq!(rejected_field(value), "validator_count");
        let value = with(report(), "/quorum", json!(0));
        assert_eq!(rejected_field(value), "quorum");
        let value = with(report(), "/quorum", json!(5));
        assert_eq!(rejected_field(value), "quorum");
    }

    #[test]
    fn rejects_policy_problems_by_field() {
        let value = with(report(), "/policy/schema", json!("postfiat-fastpay-recovery-policy-v2"));
        assert_eq!(rejected_field(value), "policy.schema");
        let value = with(report(), "/policy/activation_height", json!(0));
        assert_eq!(rejected_field(value), "policy.activation_height");
        let value = with(report(), "/policy/max_validity_blocks", json!(0));
        assert_eq!(rejected_field(value), "policy.max_validity_blocks");
        let value = with(
            report(),
            "/policy/max_validity_blocks",
            json!(MAX_FASTPAY_VALIDITY_BLOCKS + 1),
        );
        assert_eq!(rejected_field(value), "policy.max_validity_blocks");
        let value = with(report(), "/policy/max_recovery_blocks", json!(0));
        assert_eq!(rejected_field(value), "policy.max_recovery_blocks");
        let value = with(
            report(),
            "/policy/max_recovery_blocks",
            json!(MAX_FASTPAY_RECOVERY_BLOCKS + 1),
        );
        assert_eq!(rejected_field(value), "policy.max_recovery_blocks");
    }

    #[test]
    fn rejects_unknown_fields_where_the_types_do() {
        // FastPayRecoveryCapabilitiesV1 and FastPayRecoveryPolicyV1 are
        // deny_unknown_fields; OwnedCertificateDomain is not, so an extra
        // domain field is accepted exactly as the node's own type accepts it.
        let mut value = report();
        value
            .as_object_mut()
            .expect("object")
            .insert("extra".to_string(), json!(true));
        assert_eq!(rejected_field(value), "result");
        let mut value = report();
        value["domain"]
            .as_object_mut()
            .expect("domain object")
            .insert("extra".to_string(), json!(true));
        validate(&value).expect("extra domain field is tolerated by the type");
        let mut value = report();
        value["policy"]
            .as_object_mut()
            .expect("policy object")
            .insert("extra".to_string(), json!(true));
        assert_eq!(rejected_field(value), "result");
    }

    #[test]
    fn rejects_missing_sections() {
        let mut value = report();
        value.as_object_mut().expect("object").remove("domain");
        assert_eq!(rejected_field(value), "domain");
        let mut value = report();
        value.as_object_mut().expect("object").remove("policy");
        assert_eq!(rejected_field(value), "policy");
    }
}
