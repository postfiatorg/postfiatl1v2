use reserve_proof_types::yolo_nitro::{verify_nitro_document, NitroVerifier};
use reserve_proof_types::yolo_witness::NitroProofPolicyV1;

#[test]
fn real_aws_indefinite_payload_and_adjacent_rejections() {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/yolo_nitro_aws_20260905.json")).unwrap();
    for case in value["cases"].as_array().unwrap() {
        let policy: NitroProofPolicyV1 = serde_json::from_value(case["policy"].clone()).unwrap();
        let result = verify_nitro_document(
            &hex::decode(case["documentHex"].as_str().unwrap()).unwrap(),
            &hex::decode(case["rootDerHex"].as_str().unwrap()).unwrap(),
            &policy,
            case["verificationTimeMs"].as_u64().unwrap(),
            case["bindingDigest"].as_str().unwrap(),
        );
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{}: {result:?}",
            case["name"]
        );
    }
}

#[test]
fn shared_python_rust_nitro_matrix_validates_certificates_and_protocol() {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/yolo_nitro_validation_v2.json")).unwrap();
    let cases = value["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 38);
    for case in cases {
        let policy: NitroProofPolicyV1 = serde_json::from_value(case["policy"].clone()).unwrap();
        let result = verify_nitro_document(
            &hex::decode(case["documentHex"].as_str().unwrap()).unwrap(),
            &hex::decode(case["rootDerHex"].as_str().unwrap()).unwrap(),
            &policy,
            case["verificationTimeMs"].as_u64().unwrap(),
            case["bindingDigest"].as_str().unwrap(),
        );
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{}: {result:?}",
            case["name"]
        );
    }
}

#[test]
fn cached_chains_preserve_document_and_certificate_rejections() {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/yolo_nitro_validation_v2.json")).unwrap();
    let cases = value["cases"].as_array().unwrap();
    let first = &cases[0];
    let policy: NitroProofPolicyV1 = serde_json::from_value(first["policy"].clone()).unwrap();
    let root = hex::decode(first["rootDerHex"].as_str().unwrap()).unwrap();
    let mut verifier = NitroVerifier::new(
        &root,
        &policy,
        first["verificationTimeMs"].as_u64().unwrap(),
    )
    .unwrap();
    for case in cases
        .iter()
        .filter(|case| case["rootDerHex"] == first["rootDerHex"])
    {
        let result = verifier.verify(
            &hex::decode(case["documentHex"].as_str().unwrap()).unwrap(),
            case["bindingDigest"].as_str().unwrap(),
        );
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "cached {}: {result:?}",
            case["name"]
        );
    }
}
