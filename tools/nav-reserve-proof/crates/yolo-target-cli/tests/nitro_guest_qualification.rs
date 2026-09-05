#![cfg(feature = "sp1")]

use reserve_proof_types::yolo_witness::NitroProofPolicyV1;
use sha2::{Digest, Sha256};
use sp1_sdk::{Elf, Prover, ProverClient, SP1Stdin};
use std::{fs, path::Path, time::Instant};

#[test]
#[ignore = "requires the separately built Nitro qualification harness ELF"]
fn all_shared_nitro_cases_match_inside_sp1_with_pinned_crypto_patches() {
    let path = std::env::var("YOLO_NITRO_TEST_GUEST_ELF").expect("set YOLO_NITRO_TEST_GUEST_ELF");
    let hash = std::env::var("YOLO_NITRO_TEST_ELF_SHA256").expect("set YOLO_NITRO_TEST_ELF_SHA256");
    let elf = fs::read(path).unwrap();
    assert_eq!(hex::encode(Sha256::digest(&elf)), hash);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../reserve-proof-types/tests/fixtures/yolo_nitro_validation_v2.json");
    let corpus: serde_json::Value = serde_json::from_slice(&fs::read(&fixture).unwrap()).unwrap();
    let hardware: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.with_file_name("yolo_nitro_aws_20260905.json")).unwrap(),
    )
    .unwrap();
    let mut combined = corpus["cases"].as_array().unwrap().clone();
    combined.extend(hardware["cases"].as_array().unwrap().iter().cloned());
    let cases = &combined;
    assert_eq!(cases.len(), 44);
    let input: Vec<_> = cases
        .iter()
        .map(|case| {
            (
                hex::decode(case["documentHex"].as_str().unwrap()).unwrap(),
                hex::decode(case["rootDerHex"].as_str().unwrap()).unwrap(),
                serde_json::from_value::<NitroProofPolicyV1>(case["policy"].clone()).unwrap(),
                case["verificationTimeMs"].as_u64().unwrap(),
                case["bindingDigest"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let expected: Vec<_> = cases
        .iter()
        .map(|case| u8::from(case["valid"].as_bool().unwrap()))
        .collect();
    let mut stdin = SP1Stdin::new();
    let encoded = serde_cbor::to_vec(&input).unwrap();
    reserve_proof_types::yolo_cbor::decode_strict_cbor(
        &encoded,
        reserve_proof_types::yolo_cbor::CborLimits {
            bytes: 4 * 1024 * 1024,
            items: 200_000,
            depth: 32,
        },
    )
    .expect("corpus must fit the qualification guest input bounds");
    stdin.write_vec(encoded);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let client = ProverClient::builder().cpu().build().await;
        let start = Instant::now();
        let (actual, report) = client.execute(Elf::from(elf), stdin).await.unwrap();
        assert_eq!(actual.to_vec(), expected);
        for case in cases {
            println!("{}", serde_json::json!({"case":case["name"], "valid":case["valid"], "guestMatches":true}));
        }
        println!("{}", serde_json::json!({"caseCount":cases.len(),
            "instructions":report.total_instruction_count(), "elapsedMilliseconds":start.elapsed().as_millis()}));
    });
}
