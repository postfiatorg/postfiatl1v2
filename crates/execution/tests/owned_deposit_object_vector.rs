//! The FastPay object ID a signed deposit creates, pinned by the vector the
//! Python wallet also checks (python/tests/test_fastpay_wrap_attribution.py).

#[test]
fn owned_deposit_object_id_matches_the_shared_vector() {
    let vector: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/owned_deposit_object_v1.json")).unwrap();
    let deposit: postfiat_types::OwnedDepositV1 =
        serde_json::from_value(vector["deposit"].clone()).unwrap();
    assert_eq!(
        postfiat_execution::owned_deposit_object_id(&deposit).unwrap(),
        vector["object_id"].as_str().unwrap()
    );
}
