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
                cursor: None,
                omit_owner_keys: false,
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
                cursor: None,
                omit_owner_keys: false,
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
    fn accepts_the_cursor_and_owner_key_omission_in_a_request() {
        let cursor = format!("ooc2.{}.-.{}.505654.{}.1", "aa".repeat(16), "cc".repeat(32), "01".repeat(32));
        let raw = RpcRequest::empty("owned-objects-raw", METHOD_OWNED_OBJECTS)
            .with_param_value("owner_public_key_hex", string_value(owner()))
            .with_param_value("cursor", string_value(cursor.clone()))
            .with_param_value("omit_owner_keys", Value::Bool(true));
        validate_request(&raw, None, Some(RpcRequestKind::OwnedObjects))
            .expect("cursor and omit_owner_keys are accepted");
        let rejects = |name: &str, value: Value| {
            let request = RpcRequest::empty("owned-objects-bad", METHOD_OWNED_OBJECTS)
                .with_param_value("owner_public_key_hex", string_value(owner()))
                .with_param_value(name, value);
            assert!(
                validate_request(&request, None, Some(RpcRequestKind::OwnedObjects)).is_err(),
                "{name}"
            );
        };
        rejects("cursor", json!(""));
        rejects("cursor", json!(7));
        rejects("cursor", json!(" ooc2"));
        rejects("omit_owner_keys", json!("yes"));
    }

    #[test]
    fn builds_a_continuation_request_with_the_typed_params() {
        let cursor = format!("ooc2.{}.-.{}.505654.{}.1", "aa".repeat(16), "cc".repeat(32), "01".repeat(32));
        let request = owned_objects_request(
            "owned-objects-next",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: None,
                limit: Some(128),
                cursor: Some(cursor.clone()),
                omit_owner_keys: true,
            },
        );
        assert_eq!(request.params["cursor"], json!(cursor));
        assert_eq!(request.params["omit_owner_keys"], json!(true));
        validate_request(&request, None, Some(RpcRequestKind::OwnedObjects)).expect("typed request");
        let first = owned_objects_request(
            "owned-objects-first",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: None,
                limit: None,
                cursor: None,
                omit_owner_keys: false,
            },
        );
        assert!(first.params.get("cursor").is_none());
        assert!(first.params.get("omit_owner_keys").is_none());
    }

    #[test]
    fn requires_a_view_digest_on_every_paged_answer() {
        let mut missing = paged_fixture();
        missing.as_object_mut().unwrap().remove("view_digest");
        assert_eq!(rejected_field(missing), "view_digest");
        assert_eq!(
            rejected_field(with(paged_fixture(), "/view_digest", json!("CC".repeat(32)))),
            "view_digest"
        );
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
                cursor: None,
                omit_owner_keys: false,
            },
        ));
        rejects(owned_objects_request(
            "owned-objects-7",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: None,
                limit: Some(0),
                cursor: None,
                omit_owner_keys: false,
            },
        ));
        rejects(owned_objects_request(
            "owned-objects-8",
            OwnedObjectsParams {
                owner_public_key_hex: owner(),
                asset: None,
                limit: Some(2049),
                cursor: None,
                omit_owner_keys: false,
            },
        ));
        rejects(
            owned_objects_request(
                "owned-objects-9",
                OwnedObjectsParams {
                    owner_public_key_hex: owner(),
                    asset: None,
                    limit: None,
                    cursor: None,
                    omit_owner_keys: false,
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

    // --- request binding (validate_owned_objects_response) ---

    fn request_for(id: &str, owner: String, asset: Option<&str>, limit: Option<usize>) -> RpcRequest {
        owned_objects_request(
            id,
            OwnedObjectsParams {
                owner_public_key_hex: owner,
                asset: asset.map(str::to_string),
                limit,
                cursor: None,
                omit_owner_keys: false,
            },
        )
    }

    #[test]
    fn binds_a_continuation_to_its_cursor_and_key_omission() {
        let cursor = format!("ooc2.{}.-.{}.505654.{}.1", "aa".repeat(16), "cc".repeat(32), "01".repeat(32));
        let request = |cursor: Option<&str>, omit: bool| {
            owned_objects_request(
                "owned-objects-1",
                OwnedObjectsParams {
                    owner_public_key_hex: owner(),
                    asset: None,
                    limit: Some(5),
                    cursor: cursor.map(str::to_string),
                    omit_owner_keys: omit,
                },
            )
        };
        let page = with(paged_fixture(), "/cursor", json!(cursor));
        bind(&page, &request(Some(&cursor), false)).expect("the page answers its cursor");
        assert_eq!(bind_rejected_field(&paged_fixture(), &request(Some(&cursor), false)), "cursor");
        assert_eq!(bind_rejected_field(&page, &request(Some(&cursor), true)), "owner_keys_omitted");
    }

    fn bind(report: &Value, request: &RpcRequest) -> Result<(), RpcResponseValidationError> {
        validate_owned_objects_response(
            &success_response("owned-objects-1", report, vec![]).expect("owned objects response"),
            request,
        )
    }

    fn bind_rejected_field(report: &Value, request: &RpcRequest) -> String {
        match bind(report, request) {
            Err(RpcResponseValidationError::InvalidResult { field, .. }) => field,
            other => panic!("expected an InvalidResult rejection, got {other:?}"),
        }
    }

    #[test]
    fn binds_reports_to_their_matching_requests() {
        bind(&fixture(), &request_for("owned-objects-1", owner(), None, None))
            .expect("no-filter request, node default limit");
        bind(
            &fixture(),
            &request_for("owned-objects-1", owner(), None, Some(2048)),
        )
        .expect("explicit limit equal to the node default");
        bind(
            &truncated_fixture(),
            &request_for("owned-objects-1", owner(), None, Some(2)),
        )
        .expect("explicit limit 2");
        bind(
            &filtered_fixture(),
            &request_for("owned-objects-1", owner(), Some("USDX"), None),
        )
        .expect("asset-filtered request");
        let mut absent_asset = fixture();
        absent_asset.as_object_mut().expect("object").remove("asset");
        bind(
            &absent_asset,
            &request_for("owned-objects-1", owner(), None, None),
        )
        .expect("absent asset field for a no-filter request");
    }

    #[test]
    fn rejects_same_id_report_for_another_owner() {
        // Valid, internally consistent report for owner A; request selects owner B.
        let request_b = request_for("owned-objects-1", "cd".repeat(1952), None, None);
        assert!(validate(&fixture()).is_ok(), "report is valid on its own");
        assert_eq!(
            bind_rejected_field(&fixture(), &request_b),
            "owner_public_key_hex"
        );
        // The node echoes the requested string and filters by exact match, so a
        // case-only difference is a different query.
        let request_upper = request_for("owned-objects-1", "AB".repeat(1952), None, None);
        assert_eq!(
            bind_rejected_field(&fixture(), &request_upper),
            "owner_public_key_hex"
        );
    }

    #[test]
    fn rejects_same_id_report_for_another_asset_or_filter() {
        // Filtered USDX report against a PFT request.
        let request_pft = request_for("owned-objects-1", owner(), Some("PFT"), None);
        assert!(validate(&filtered_fixture()).is_ok());
        assert_eq!(bind_rejected_field(&filtered_fixture(), &request_pft), "asset");
        // Filtered report against a no-filter request.
        let request_none = request_for("owned-objects-1", owner(), None, None);
        assert_eq!(bind_rejected_field(&filtered_fixture(), &request_none), "asset");
        // No-filter report (asset null, or absent) against a filtered request.
        let request_usdx = request_for("owned-objects-1", owner(), Some("USDX"), None);
        assert_eq!(bind_rejected_field(&fixture(), &request_usdx), "asset");
        let mut absent_asset = fixture();
        absent_asset.as_object_mut().expect("object").remove("asset");
        assert_eq!(bind_rejected_field(&absent_asset, &request_usdx), "asset");
    }

    #[test]
    fn rejects_same_id_report_for_another_limit() {
        // Default-limit report against an explicit limit 25.
        let request_25 = request_for("owned-objects-1", owner(), None, Some(25));
        assert_eq!(bind_rejected_field(&fixture(), &request_25), "limit");
        // Limit-2 report against a request that omitted limit (node default 2048).
        let request_default = request_for("owned-objects-1", owner(), None, None);
        assert!(validate(&truncated_fixture()).is_ok());
        assert_eq!(
            bind_rejected_field(&truncated_fixture(), &request_default),
            "limit"
        );
    }

    #[test]
    fn rejects_unbound_response_id_foreign_method_and_malformed_request() {
        let other_id = request_for("owned-objects-2", owner(), None, None);
        assert!(matches!(
            bind(&fixture(), &other_id),
            Err(RpcResponseValidationError::UnexpectedId { .. })
        ));
        let foreign = RpcRequest::empty("owned-objects-1", METHOD_ACCOUNT_ASSETS)
            .with_param_value("owner_public_key_hex", string_value(owner()));
        assert_eq!(bind_rejected_field(&fixture(), &foreign), "request");
        let malformed = request_for("owned-objects-1", "ab".repeat(1951), None, None);
        assert_eq!(bind_rejected_field(&fixture(), &malformed), "request");
        let zero_limit = request_for("owned-objects-1", owner(), None, Some(0));
        assert_eq!(bind_rejected_field(&fixture(), &zero_limit), "request");
    }

    /// A cursor-era page: the node's byte bound ended it at 2 of limit 5.
    fn paged_fixture() -> Value {
        let mut report = with(truncated_fixture(), "/limit", json!(5));
        let map = report.as_object_mut().unwrap();
        map.insert("cursor".into(), Value::Null);
        map.insert("next_cursor".into(), json!("ooc2.opaque"));
        map.insert("height".into(), json!(40));
        map.insert("state_root".into(), json!("bb".repeat(48)));
        map.insert("view_digest".into(), json!("cc".repeat(32)));
        report
    }

    #[test]
    fn accepts_byte_bounded_continued_and_keyless_pages() {
        assert!(validate(&paged_fixture()).is_ok());
        // The last page after a cursor holds part of the summed value.
        let last = with(paged_fixture(), "/cursor", json!("ooc2.opaque"));
        let last = with(last, "/next_cursor", Value::Null);
        let last = with(last, "/truncated", json!(false));
        assert!(validate(&last).is_ok());
        let mut keyless = paged_fixture();
        keyless["owner_keys_omitted"] = json!(true);
        for row in keyless["objects"].as_array_mut().unwrap() {
            row.as_object_mut().unwrap().remove("owner_pubkey_hex");
        }
        assert!(validate(&keyless).is_ok());
        // A keyless page must not carry a per-object key.
        let mut keyed = paged_fixture();
        keyed["owner_keys_omitted"] = json!(true);
        assert_eq!(rejected_field(keyed), "objects[0].owner_pubkey_hex");
        // next_cursor and truncated must agree.
        assert_eq!(
            rejected_field(with(paged_fixture(), "/next_cursor", Value::Null)),
            "next_cursor"
        );
        // A first page without a cursor still sums exactly when complete.
        let first = with(last_unpaged(), "/total_value", json!(999));
        assert_eq!(rejected_field(first), "total_value");
    }

    fn last_unpaged() -> Value {
        let mut report = fixture();
        let map = report.as_object_mut().unwrap();
        map.insert("cursor".into(), Value::Null);
        map.insert("next_cursor".into(), Value::Null);
        map.insert("height".into(), json!(40));
        map.insert("state_root".into(), json!("bb".repeat(48)));
        map.insert("view_digest".into(), json!("cc".repeat(32)));
        report
    }

    /// A revision-era page: `inventory_revision` and `node_id` beside the
    /// cursor fields. Older nodes omit both and stay valid.
    fn revision_fixture() -> Value {
        let mut report = paged_fixture();
        let map = report.as_object_mut().unwrap();
        map.insert(
            "inventory_revision".into(),
            json!({
                "finalized_height": 40,
                "fastpay_sequence": 3,
                "incarnation": "0f".repeat(16),
            }),
        );
        map.insert("node_id".into(), json!("validator-0"));
        report
    }

    #[test]
    fn accepts_and_checks_the_inventory_revision_and_node_id() {
        assert!(validate(&revision_fixture()).is_ok());
        assert!(validate(&paged_fixture()).is_ok(), "older nodes omit both");
        let mut no_id = revision_fixture();
        no_id.as_object_mut().unwrap().remove("node_id");
        assert!(validate(&no_id).is_ok());
        assert_eq!(
            rejected_field(with(
                revision_fixture(),
                "/inventory_revision/finalized_height",
                json!(41)
            )),
            "inventory_revision.finalized_height"
        );
        assert_eq!(
            rejected_field(with(
                revision_fixture(),
                "/inventory_revision/fastpay_sequence",
                json!(-1)
            )),
            "inventory_revision.fastpay_sequence"
        );
        assert_eq!(
            rejected_field(with(revision_fixture(), "/inventory_revision", json!(7))),
            "inventory_revision"
        );
        assert_eq!(
            rejected_field(with(revision_fixture(), "/node_id", json!(" v0"))),
            "node_id"
        );
    }

    /// The incarnation is optional, lowercase hex of 16 bytes; a page
    /// without a revision may say why, and never both.
    #[test]
    fn accepts_and_checks_the_incarnation_and_the_unavailable_reason() {
        let mut no_incarnation = revision_fixture();
        no_incarnation["inventory_revision"]
            .as_object_mut()
            .unwrap()
            .remove("incarnation");
        assert!(validate(&no_incarnation).is_ok());
        for bad in [json!("0F".repeat(16)), json!("0f".repeat(15)), json!(7)] {
            assert_eq!(
                rejected_field(with(revision_fixture(), "/inventory_revision/incarnation", bad)),
                "inventory_revision.incarnation"
            );
        }
        let mut unavailable = revision_fixture();
        let map = unavailable.as_object_mut().unwrap();
        map.remove("inventory_revision");
        map.insert(
            "inventory_revision_unavailable".into(),
            json!("legacy_owned_applies_enabled"),
        );
        assert!(validate(&unavailable).is_ok());
        assert_eq!(
            rejected_field(with(unavailable.clone(), "/inventory_revision_unavailable", json!(""))),
            "inventory_revision_unavailable"
        );
        let mut both = revision_fixture();
        both.as_object_mut().unwrap().insert(
            "inventory_revision_unavailable".into(),
            json!("legacy_owned_applies_enabled"),
        );
        assert_eq!(rejected_field(both), "inventory_revision_unavailable");
    }
}
