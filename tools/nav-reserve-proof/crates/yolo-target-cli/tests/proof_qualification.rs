#![cfg(feature = "sp1")]

use bincode::Options;
use postfiat_types::YoloTargetPublicValuesV1;
use reserve_proof_types::yolo_witness::TargetAcceptanceV1;
use sha2::{Digest, Sha256};
use sp1_sdk::{
    Elf, Prover, ProverClient, ProvingKey, SP1Proof, SP1ProofWithPublicValues, SP1PublicValues,
};
use std::{fs, time::Instant};

#[test]
#[ignore = "requires a completed synthetic Groth16 proof and independently pinned ELF/manifest"]
fn groth16_binds_public_values_and_program_and_rejects_proof_corruption() {
    let proof_path = std::env::var("YOLO_TARGET_PROOF").expect("set YOLO_TARGET_PROOF");
    let elf_path = std::env::var("YOLO_TARGET_GUEST_ELF").expect("set YOLO_TARGET_GUEST_ELF");
    let other_path = std::env::var("YOLO_OTHER_GUEST_ELF").expect("set YOLO_OTHER_GUEST_ELF");
    let pins_path =
        std::env::var("YOLO_TARGET_ACCEPTANCE").expect("set independent YOLO_TARGET_ACCEPTANCE");
    let pins: TargetAcceptanceV1 = serde_json::from_slice(&fs::read(pins_path).unwrap()).unwrap();
    let elf = fs::read(elf_path).unwrap();
    assert_eq!(hex::encode(Sha256::digest(&elf)), pins.program_sha256);
    let encoded = fs::read(proof_path).unwrap();
    let proof: SP1ProofWithPublicValues = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .with_limit(32 * 1024 * 1024)
        .deserialize(&encoded)
        .unwrap();
    assert!(matches!(&proof.proof, SP1Proof::Groth16(_)));
    assert!(proof.tee_proof.is_none());
    let public = proof.public_values.to_vec();
    assert_eq!(public.len(), 408);
    pins.verify(&YoloTargetPublicValuesV1::decode(&public).unwrap())
        .unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let client = ProverClient::builder().light().build().await;
        let key = client.setup(Elf::from(elf)).await.unwrap();
        let start = Instant::now();
        client.verify(&proof, key.verifying_key(), None).unwrap();
        println!(
            "{}",
            serde_json::json!({"case":"valid_groth16", "verified":true,
            "verificationMicroseconds":start.elapsed().as_micros()})
        );
        for (name, offset) in [
            ("program_commitment", 12),
            ("manifest_commitment", 108),
            ("target_commitment", 364),
        ] {
            let mut changed = proof.clone();
            let mut bytes = public.clone();
            bytes[offset] ^= 1;
            changed.public_values = SP1PublicValues::from(&bytes);
            assert!(
                client.verify(&changed, key.verifying_key(), None).is_err(),
                "accepted {name}"
            );
            println!("{}", serde_json::json!({"case":name, "rejected":true}));
        }
        let mut changed = proof.clone();
        changed.public_values = SP1PublicValues::from(&public[..407]);
        assert!(client.verify(&changed, key.verifying_key(), None).is_err());
        println!(
            "{}",
            serde_json::json!({"case":"truncated_public_values", "rejected":true})
        );
        let mut changed = proof.clone();
        let SP1Proof::Groth16(inner) = &mut changed.proof else {
            unreachable!()
        };
        inner.encoded_proof = "00".repeat(inner.encoded_proof.len() / 2);
        assert!(client.verify(&changed, key.verifying_key(), None).is_err());
        println!(
            "{}",
            serde_json::json!({"case":"corrupted_groth16", "rejected":true})
        );
        let other = client
            .setup(Elf::from(fs::read(other_path).unwrap()))
            .await
            .unwrap();
        assert!(client.verify(&proof, other.verifying_key(), None).is_err());
        println!(
            "{}",
            serde_json::json!({"case":"wrong_program_verification_key", "rejected":true})
        );
    });
}
