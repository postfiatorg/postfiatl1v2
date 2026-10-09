mod owned_objects {
    use super::*;
    use serde_json::{Value, json};

    fn owner() -> String {
        "ab".repeat(1952)
    }

    fn with(mut value: Value, pointer: &str, replacement: Value) -> Value {
        *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
        value
    }

    fn object(id_byte: &str, version: u64, value: u64, asset: &str) -> Value {
        json!({
            "id": id_byte.repeat(32),
            "version": version,
            "owner_pubkey_hex": owner(),
            "value": value,
            "asset": asset
        })
    }

    /// An untruncated page: three objects already in the node's
    /// (asset, id, version) order, total_value equal to their sum.
    fn fixture() -> Value {
        json!({
            "schema": "postfiat-owned-objects-v1",
            "chain_id": "postfiat-testnet",
            "genesis_hash": "cd".repeat(48),
            "protocol_version": 3,
            "owner_public_key_hex": owner(),
            "asset": null,
            "limit": 2048,
            "truncated": false,
            "object_count": 3,
            "total_value": 600,
            "objects": [
                object("01", 1, 100, "PFT"),
                object("02", 4, 200, "PFT"),
                object("01", 2, 300, "USDX")
            ]
        })
    }

    /// A full page cut off by the limit; the node summed before truncating.
    fn truncated_fixture() -> Value {
        let report = with(fixture(), "/limit", json!(2));
        let report = with(report, "/truncated", json!(true));
        let report = with(report, "/object_count", json!(2));
        let report = with(report, "/total_value", json!(600));
        with(
            report,
            "/objects",
            json!([object("01", 1, 100, "PFT"), object("02", 4, 200, "PFT")]),
        )
    }

    /// A page filtered to one asset.
    fn filtered_fixture() -> Value {
        let report = with(fixture(), "/asset", json!("USDX"));
        let report = with(report, "/object_count", json!(1));
        let report = with(report, "/total_value", json!(300));
        with(report, "/objects", json!([object("01", 2, 300, "USDX")]))
    }

    fn validate(report: &Value) -> Result<(), RpcResponseValidationError> {
        validate_response_kind(
            &success_response("owned-objects-1", report, vec![]).expect("owned objects response"),
            RpcResponseKind::OwnedObjects,
        )
    }

    fn rejected_field(report: Value) -> String {
        match validate(&report) {
            Err(RpcResponseValidationError::InvalidResult { field, .. }) => field,
            other => panic!("expected an InvalidResult rejection, got {other:?}"),
        }
    }

    #[test]
    fn builds_and_validates_owned_objects_request_without_optional_params() {
        let request = owned_objects_request(
            "owned-objects-1",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: None,
                limit: None,
            },
        );
        assert_eq!(request.method, METHOD_OWNED_OBJECTS);
        assert_eq!(request.params["owner_public_key_hex"], json!(owner()));
        assert!(request.params.get("asset").is_none());
        assert!(request.params.get("limit").is_none());
        validate_request(
            &request,
            Some("owned-objects-1"),
            Some(RpcRequestKind::OwnedObjects),
        )
        .expect("owned objects request");
    }

    #[test]
    fn builds_and_validates_owned_objects_request_with_optional_params() {
        let request = owned_objects_request(
            "owned-objects-2",
            OwnedObjectsParams {
                owner_public_key_hex: "AB".repeat(1952),
                asset: Some("USDX".to_string()),
                limit: Some(2048),
            },
        );
        assert_eq!(request.params["asset"], json!("USDX"));
        assert_eq!(request.params["limit"], json!(2048));
        validate_request(
            &request,
            Some("owned-objects-2"),
            Some(RpcRequestKind::OwnedObjects),
        )
        .expect("owned objects request with uppercase key, asset, and limit");
    }

    #[test]
    fn rejects_owned_objects_request_params() {
        let rejects = |request: RpcRequest| {
            assert!(
                validate_request(&request, None, Some(RpcRequestKind::OwnedObjects)).is_err(),
                "expected rejection for {:?}",
                request.params
            );
        };
        rejects(RpcRequest::empty("owned-objects-3", METHOD_OWNED_OBJECTS));
        rejects(
            RpcRequest::empty("owned-objects-4", METHOD_OWNED_OBJECTS)
                .with_param_value("owner_public_key_hex", string_value("ab".repeat(1951))),
        );
        rejects(
            RpcRequest::empty("owned-objects-5", METHOD_OWNED_OBJECTS)
                .with_param_value("owner_public_key_hex", string_value("zz".repeat(1952))),
        );
        rejects(owned_objects_request(
            "owned-objects-6",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: Some(" USDX".to_string()),
                limit: None,
            },
        ));
        rejects(owned_objects_request(
            "owned-objects-7",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: None,
                limit: Some(0),
            },
        ));
        rejects(owned_objects_request(
            "owned-objects-8",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: None,
                limit: Some(2049),
            },
        ));
        rejects(
            owned_objects_request(
                "owned-objects-9",
                OwnedObjectsParams {
                    owner_public_key_hex: owner(),
                    asset: None,
                    limit: None,
                },
            )
            .with_param_value("state", string_value("open")),
        );
    }

    #[test]
    fn accepts_untruncated_truncated_and_filtered_reports() {
        validate(&fixture()).expect("untruncated report");
        validate(&truncated_fixture()).expect("truncated report");
        validate(&filtered_fixture()).expect("asset-filtered report");
        let absent_asset = {
            let mut report = fixture();
            report.as_object_mut().expect("object").remove("asset");
            report
        };
        validate(&absent_asset).expect("report without an asset field");
    }

    #[test]
    fn accepts_mixed_case_identifiers() {
        let report = with(fixture(), "/owner_public_key_hex", json!("AB".repeat(1952)));
        let report = with(report, "/objects/0/owner_pubkey_hex", json!("AB".repeat(1952)));
        let report = with(report, "/objects/1/owner_pubkey_hex", json!("AB".repeat(1952)));
        let report = with(report, "/objects/2/owner_pubkey_hex", json!("AB".repeat(1952)));
        let report = with(report, "/objects/2/id", json!("0A".repeat(32)));
        validate(&report).expect("mixed-case hex");
    }

    #[test]
    fn rejects_wrong_schema() {
        let report = with(fixture(), "/schema", json!("postfiat-owned-objects-v2"));
        assert_eq!(rejected_field(report), "schema");
    }

    #[test]
    fn rejects_malformed_owner_key() {
        let report = with(fixture(), "/owner_public_key_hex", json!("ab".repeat(1951)));
        assert_eq!(rejected_field(report), "owner_public_key_hex");
    }

    #[test]
    fn rejects_limit_outside_node_bound() {
        let report = with(fixture(), "/limit", json!(0));
        assert_eq!(rejected_field(report), "limit");
        let report = with(fixture(), "/limit", json!(2049));
        assert_eq!(rejected_field(report), "limit");
    }

    #[test]
    fn rejects_object_count_that_disagrees_with_rows_or_limit() {
        let report = with(fixture(), "/object_count", json!(2));
        assert_eq!(rejected_field(report), "object_count");
        let report = with(fixture(), "/limit", json!(2));
        assert_eq!(rejected_field(report), "object_count");
    }

    #[test]
    fn rejects_truncated_flag_on_a_page_that_is_not_full() {
        let report = with(fixture(), "/truncated", json!(true));
        assert_eq!(rejected_field(report), "truncated");
    }

    #[test]
    fn rejects_objects_owned_by_someone_else() {
        let report = with(fixture(), "/objects/1/owner_pubkey_hex", json!("cd".repeat(1952)));
        assert_eq!(rejected_field(report), "objects[1].owner_pubkey_hex");
    }

    #[test]
    fn rejects_objects_outside_the_requested_asset() {
        let report = with(filtered_fixture(), "/objects/0/asset", json!("PFT"));
        assert_eq!(rejected_field(report), "objects[0].asset");
    }

    #[test]
    fn rejects_objects_out_of_node_sort_order() {
        let swapped = with(
            fixture(),
            "/objects",
            json!([
                object("02", 4, 200, "PFT"),
                object("01", 1, 100, "PFT"),
                object("01", 2, 300, "USDX")
            ]),
        );
        assert_eq!(rejected_field(swapped), "objects[1]");
        let asset_order = with(
            fixture(),
            "/objects",
            json!([
                object("01", 2, 300, "USDX"),
                object("01", 1, 100, "PFT"),
                object("02", 4, 200, "PFT")
            ]),
        );
        assert_eq!(rejected_field(asset_order), "objects[1]");
    }

    #[test]
    fn rejects_total_value_that_disagrees_with_returned_values() {
        let report = with(fixture(), "/total_value", json!(601));
        assert_eq!(rejected_field(report), "total_value");
        let report = with(truncated_fixture(), "/total_value", json!(299));
        assert_eq!(rejected_field(report), "total_value");
    }

    #[test]
    fn rejects_malformed_object_rows() {
        let report = with(fixture(), "/objects/0/id", json!("01".repeat(31)));
        assert_eq!(rejected_field(report), "id");
        let report = with(fixture(), "/objects/0/value", json!(-5));
        assert_eq!(rejected_field(report), "value");
        let report = with(fixture(), "/objects/0/asset", json!(""));
        assert_eq!(rejected_field(report), "asset");
    }
}
