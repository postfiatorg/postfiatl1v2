mod owned_certificate {
    use super::*;
    use postfiat_types::{
        FastPayCertificateV1, FastPayOrderRecoveryV1, OwnedCertificateDomain, OwnedObjectRef,
        OwnedOutputSpec, OwnedTransferCertificateV3, OwnedTransferOrderV3, OwnedTransferVote,
        OwnedUnwrapCertificateV3, OwnedUnwrapOrderV3, OwnedUnwrapVote,
        FASTPAY_ORDER_RECOVERY_SCHEMA_V1, OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
    };
    use serde_json::{Value, json};

    fn with(mut value: Value, pointer: &str, replacement: Value) -> Value {
        *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
        value
    }

    fn domain() -> OwnedCertificateDomain {
        OwnedCertificateDomain {
            schema: OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
            chain_id: "rpc-sdk-tests".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 3,
            registry_id: "22".repeat(48),
        }
    }

    fn recovery() -> FastPayOrderRecoveryV1 {
        FastPayOrderRecoveryV1 {
            schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
            committee_epoch: 7,
            lock_id: "00".repeat(48),
            valid_from_height: 100,
            expires_at_height: 110,
            recovery_closes_at_height: 120,
        }
    }

    /// A transfer certificate whose lock id is the real
    /// `fastpay_transfer_lock_id_v1` digest of its order.
    fn transfer_certificate() -> FastPayCertificateV1 {
        let mut order = OwnedTransferOrderV3 {
            domain: domain(),
            recovery: recovery(),
            inputs: vec![OwnedObjectRef {
                id: "33".repeat(32),
                version: 9,
            }],
            outputs: vec![OwnedOutputSpec {
                owner_pubkey_hex: "44".repeat(32),
                value: 9,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 8,
            memos: Vec::new(),
        };
        order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
        FastPayCertificateV1::Transfer(OwnedTransferCertificateV3 {
            order,
            owner_pubkey_hex: "aa".repeat(32),
            owner_signature_hex: "bb".repeat(32),
            votes: vec![OwnedTransferVote {
                validator_id: "validator-0".to_string(),
                signature_hex: "cc".repeat(32),
            }],
        })
    }

    /// An unwrap certificate whose lock id is the real
    /// `fastpay_unwrap_lock_id_v1` digest of its order.
    fn unwrap_certificate() -> FastPayCertificateV1 {
        let mut order = OwnedUnwrapOrderV3 {
            domain: domain(),
            recovery: recovery(),
            inputs: vec![OwnedObjectRef {
                id: "55".repeat(32),
                version: 2,
            }],
            to_address: "pfdestination".to_string(),
            amount: 9,
            asset: "PFT".to_string(),
            fee: 1,
            nonce: 3,
            memos: Vec::new(),
        };
        order.recovery.lock_id = postfiat_types::fastpay_unwrap_lock_id_v1(&order);
        FastPayCertificateV1::Unwrap(OwnedUnwrapCertificateV3 {
            order,
            owner_pubkey_hex: "aa".repeat(32),
            owner_signature_hex: "bb".repeat(32),
            votes: vec![
                OwnedUnwrapVote {
                    validator_id: "validator-0".to_string(),
                    signature_hex: "cc".repeat(32),
                },
                OwnedUnwrapVote {
                    validator_id: "validator-1".to_string(),
                    signature_hex: "dd".repeat(32),
                },
            ],
        })
    }

    fn digest_of(certificate: &FastPayCertificateV1) -> String {
        match certificate {
            FastPayCertificateV1::Transfer(value) => {
                wallet_fastpay_transfer_certificate_digest_v3(value).expect("transfer digest")
            }
            FastPayCertificateV1::Unwrap(value) => {
                wallet_fastpay_unwrap_certificate_digest_v3(value).expect("unwrap digest")
            }
        }
    }

    fn report(certificate: &FastPayCertificateV1) -> Value {
        serde_json::to_value(certificate).expect("certificate json")
    }

    fn response(report: &Value) -> RpcResponse {
        success_response("owned-certificate-1", report, vec![]).expect("owned certificate response")
    }

    fn validate_shape(report: &Value) -> Result<(), RpcResponseValidationError> {
        validate_response_kind(&response(report), RpcResponseKind::OwnedCertificate)
    }

    fn bind(report: &Value, request: &RpcRequest) -> Result<(), RpcResponseValidationError> {
        validate_owned_certificate_response(&response(report), request)
    }

    fn rejected_field(result: Result<(), RpcResponseValidationError>) -> String {
        match result {
            Err(RpcResponseValidationError::InvalidResult { field, .. }) => field,
            other => panic!("expected an InvalidResult rejection, got {other:?}"),
        }
    }

    #[test]
    fn builds_and_validates_requests_for_both_selectors() {
        let by_lock = owned_certificate_request(
            "owned-certificate-1",
            OwnedCertificateSelector::LockId("ab".repeat(48)),
        );
        assert_eq!(by_lock.method, METHOD_OWNED_CERTIFICATE);
        assert_eq!(by_lock.params["lock_id"], json!("ab".repeat(48)));
        assert!(by_lock.params.get("certificate_digest").is_none());
        validate_request(
            &by_lock,
            Some("owned-certificate-1"),
            Some(RpcRequestKind::OwnedCertificate),
        )
        .expect("request by lock id");

        let by_digest = owned_certificate_request(
            "owned-certificate-2",
            OwnedCertificateSelector::CertificateDigest("cd".repeat(48)),
        );
        assert_eq!(by_digest.params["certificate_digest"], json!("cd".repeat(48)));
        assert!(by_digest.params.get("lock_id").is_none());
        validate_request(
            &by_digest,
            Some("owned-certificate-2"),
            Some(RpcRequestKind::OwnedCertificate),
        )
        .expect("request by certificate digest");
        assert_eq!(
            owned_certificate_selector_from_params(&by_digest.params).expect("selector"),
            OwnedCertificateSelector::CertificateDigest("cd".repeat(48))
        );
    }

    #[test]
    fn rejects_owned_certificate_request_params() {
        let rejects = |request: RpcRequest| {
            assert!(
                validate_request(&request, None, Some(RpcRequestKind::OwnedCertificate)).is_err(),
                "expected rejection for {:?}",
                request.params
            );
        };
        rejects(RpcRequest::empty("owned-certificate-3", METHOD_OWNED_CERTIFICATE));
        rejects(
            RpcRequest::empty("owned-certificate-4", METHOD_OWNED_CERTIFICATE)
                .with_param_value("lock_id", string_value("ab".repeat(48)))
                .with_param_value("certificate_digest", string_value("cd".repeat(48))),
        );
        rejects(
            RpcRequest::empty("owned-certificate-5", METHOD_OWNED_CERTIFICATE)
                .with_param_value("lock_id", string_value("AB".repeat(48))),
        );
        rejects(
            RpcRequest::empty("owned-certificate-6", METHOD_OWNED_CERTIFICATE)
                .with_param_value("certificate_digest", string_value("cd".repeat(47))),
        );
        rejects(
            owned_certificate_request(
                "owned-certificate-7",
                OwnedCertificateSelector::LockId("ab".repeat(48)),
            )
            .with_param_value("limit", u64_value(1)),
        );
    }

    #[test]
    fn accepts_transfer_and_unwrap_certificates_and_binds_both_selectors() {
        for certificate in [transfer_certificate(), unwrap_certificate()] {
            let value = report(&certificate);
            validate_shape(&value).expect("certificate shape");
            let by_lock = owned_certificate_request(
                "owned-certificate-1",
                OwnedCertificateSelector::LockId(certificate.recovery().lock_id.clone()),
            );
            bind(&value, &by_lock).expect("bound by lock id");
            let by_digest = owned_certificate_request(
                "owned-certificate-1",
                OwnedCertificateSelector::CertificateDigest(digest_of(&certificate)),
            );
            bind(&value, &by_digest).expect("bound by certificate digest");
        }
    }

    #[test]
    fn rejects_unknown_operation_tag() {
        let value = with(report(&transfer_certificate()), "/operation", json!("burn"));
        assert_eq!(rejected_field(validate_shape(&value)), "operation");
    }

    #[test]
    fn rejects_unknown_fields_where_the_types_do() {
        let mut value = report(&transfer_certificate());
        value["certificate"]
            .as_object_mut()
            .expect("certificate object")
            .insert("extra".to_string(), json!(true));
        assert_eq!(rejected_field(validate_shape(&value)), "result");
        let mut value = report(&unwrap_certificate());
        value["certificate"]["order"]
            .as_object_mut()
            .expect("order object")
            .insert("extra".to_string(), json!(true));
        assert_eq!(rejected_field(validate_shape(&value)), "result");
    }

    #[test]
    fn rejects_votes_the_type_rejects() {
        let value = with(report(&transfer_certificate()), "/certificate/votes", json!([]));
        assert_eq!(rejected_field(validate_shape(&value)), "certificate.votes");
        let value = with(
            report(&unwrap_certificate()),
            "/certificate/votes/1/validator_id",
            json!("validator-0"),
        );
        assert_eq!(rejected_field(validate_shape(&value)), "certificate.votes");
    }

    #[test]
    fn rejects_domain_problems_by_field() {
        let base = report(&transfer_certificate());
        let value = with(base.clone(), "/certificate/order/domain/schema", json!("v2"));
        assert_eq!(rejected_field(validate_shape(&value)), "certificate.order.domain.schema");
        let value = with(base.clone(), "/certificate/order/domain/chain_id", json!(" "));
        assert_eq!(rejected_field(validate_shape(&value)), "certificate.order.domain.chain_id");
        let value = with(
            base.clone(),
            "/certificate/order/domain/genesis_hash",
            json!("11".repeat(47)),
        );
        assert_eq!(
            rejected_field(validate_shape(&value)),
            "certificate.order.domain.genesis_hash"
        );
        let value = with(base.clone(), "/certificate/order/domain/protocol_version", json!(0));
        assert_eq!(
            rejected_field(validate_shape(&value)),
            "certificate.order.domain.protocol_version"
        );
        let value = with(base, "/certificate/order/domain/registry_id", json!(""));
        assert_eq!(
            rejected_field(validate_shape(&value)),
            "certificate.order.domain.registry_id"
        );
    }

    #[test]
    fn rejects_recovery_problems_by_field() {
        let base = report(&unwrap_certificate());
        let value = with(base.clone(), "/certificate/order/recovery/schema", json!("v2"));
        assert_eq!(rejected_field(validate_shape(&value)), "certificate.order.recovery.schema");
        let value = with(base.clone(), "/certificate/order/recovery/committee_epoch", json!(0));
        assert_eq!(
            rejected_field(validate_shape(&value)),
            "certificate.order.recovery.committee_epoch"
        );
        let value = with(
            base.clone(),
            "/certificate/order/recovery/lock_id",
            json!("AB".repeat(48)),
        );
        assert_eq!(rejected_field(validate_shape(&value)), "certificate.order.recovery.lock_id");
        let value = with(base.clone(), "/certificate/order/recovery/valid_from_height", json!(0));
        assert_eq!(
            rejected_field(validate_shape(&value)),
            "certificate.order.recovery.recovery_closes_at_height"
        );
        let value = with(base, "/certificate/order/recovery/recovery_closes_at_height", json!(110));
        assert_eq!(
            rejected_field(validate_shape(&value)),
            "certificate.order.recovery.recovery_closes_at_height"
        );
    }

    #[test]
    fn rejects_empty_inputs() {
        let value = with(report(&transfer_certificate()), "/certificate/order/inputs", json!([]));
        assert_eq!(rejected_field(validate_shape(&value)), "certificate.order.inputs");
    }

    #[test]
    fn rejects_certificate_for_another_lock() {
        let request = owned_certificate_request(
            "owned-certificate-1",
            OwnedCertificateSelector::LockId("ab".repeat(48)),
        );
        assert_eq!(
            rejected_field(bind(&report(&transfer_certificate()), &request)),
            "certificate.order.recovery.lock_id"
        );
    }

    #[test]
    fn rejects_certificate_with_another_digest() {
        let request = owned_certificate_request(
            "owned-certificate-1",
            OwnedCertificateSelector::CertificateDigest(digest_of(&transfer_certificate())),
        );
        assert_eq!(
            rejected_field(bind(&report(&unwrap_certificate()), &request)),
            "certificate_digest"
        );
    }

    #[test]
    fn rejects_binding_against_a_foreign_or_malformed_request() {
        let value = report(&transfer_certificate());
        let foreign = RpcRequest::empty("owned-certificate-1", METHOD_STATUS);
        assert_eq!(rejected_field(bind(&value, &foreign)), "request.method");
        let malformed = RpcRequest::empty("owned-certificate-1", METHOD_OWNED_CERTIFICATE)
            .with_param_value("lock_id", string_value("ab".repeat(48)))
            .with_param_value("certificate_digest", string_value("cd".repeat(48)));
        assert_eq!(rejected_field(bind(&value, &malformed)), "request.params");
    }

    #[test]
    fn rejects_response_whose_id_differs_from_the_request() {
        let certificate = transfer_certificate();
        let request = owned_certificate_request(
            "owned-certificate-2",
            OwnedCertificateSelector::LockId(certificate.recovery().lock_id.clone()),
        );
        match bind(&report(&certificate), &request) {
            Err(RpcResponseValidationError::UnexpectedId { expected, found }) => {
                assert_eq!(expected, "owned-certificate-2");
                assert_eq!(found, "owned-certificate-1");
            }
            other => panic!("expected an UnexpectedId rejection, got {other:?}"),
        }
    }

    #[test]
    fn binding_still_applies_the_shape_checks() {
        let request = owned_certificate_request(
            "owned-certificate-1",
            OwnedCertificateSelector::LockId(transfer_certificate().recovery().lock_id.clone()),
        );
        let value = with(report(&transfer_certificate()), "/operation", json!("burn"));
        assert_eq!(rejected_field(bind(&value, &request)), "operation");
    }
}
