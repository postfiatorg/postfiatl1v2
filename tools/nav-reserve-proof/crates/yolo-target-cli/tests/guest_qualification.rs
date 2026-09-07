#![cfg(feature = "sp1")]

use reserve_proof_types::yolo_target_proof::execute_yolo_target_proof;
use reserve_proof_types::yolo_witness::{TargetProofWitnessV1, MAX_TARGET_WITNESS_BYTES};
use sha2::{Digest, Sha256};
use sp1_sdk::{Elf, Prover, ProverClient, SP1Stdin};
use std::{fs, path::Path, time::Instant};

fn stdin(encoded: Vec<u8>) -> SP1Stdin {
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(encoded);
    stdin
}

#[test]
#[ignore = "requires the explicitly pinned built ELF; runs the full SP1 qualification matrix"]
fn guest_matches_fourteen_python_vectors_and_rejects_private_witness_mutations() {
    let elf_path = std::env::var("YOLO_TARGET_GUEST_ELF").expect("set YOLO_TARGET_GUEST_ELF");
    let elf_hash = std::env::var("YOLO_EXPECTED_ELF_SHA256")
        .expect("set YOLO_EXPECTED_ELF_SHA256 from the reviewed build");
    let elf_bytes = fs::read(elf_path).unwrap();
    assert_eq!(hex::encode(Sha256::digest(&elf_bytes)), elf_hash);
    let elf = Elf::from(elf_bytes);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../reserve-proof-types/tests/fixtures/yolo_target_proof_v1");
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 14);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let client = ProverClient::builder().cpu().build().await;
        let mut baseline = None;
        for path in paths {
            let fixture: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            let mut witness: TargetProofWitnessV1 = serde_json::from_value(fixture["witness"].clone()).unwrap();
            witness.program_sha256 = elf_hash.clone();
            let encoded = serde_cbor::to_vec(&witness).unwrap();
            let mut expected = hex::decode(fixture["expectedPublicValuesHex"].as_str().unwrap()).unwrap();
            expected[12..44].copy_from_slice(&hex::decode(&elf_hash).unwrap());
            assert_eq!(execute_yolo_target_proof(&witness).unwrap().encode().unwrap(), expected);
            let start = Instant::now();
            let (public, report) = client.execute(elf.clone(), stdin(encoded.clone())).await.unwrap();
            assert_eq!(public.to_vec(), expected, "{}", fixture["name"]);
            println!("{}", serde_json::json!({"case": fixture["name"], "valid": true, "witnessBytes": encoded.len(),
                "instructions": report.total_instruction_count(), "elapsedMilliseconds": start.elapsed().as_millis(), "gas": report.gas()}));
            if fixture["name"] == "baseline" { baseline = Some(witness); }
        }
        let baseline = baseline.unwrap();
        let mut mutants = Vec::new();
        let mut witness = baseline.clone(); witness.statements.swap(0, 1); mutants.push(("statement_order", witness));
        let mut witness = baseline.clone(); witness.statements[1].previous_statement_sha256 = None; mutants.push(("statement_chain", witness));
        let mut witness = baseline.clone(); witness.manifest.proof_policy.root_certificate_sha256 = "00".repeat(32); mutants.push(("root_policy", witness));
        let mut witness = baseline.clone(); witness.manifest.proof_policy.approved_pcr_sets[0][0].sha384 = "22".repeat(48); mutants.push(("pcr_policy", witness));
        let mut witness = baseline.clone(); witness.manifest.proof_policy.max_age_ms = 1; mutants.push(("time_policy", witness));
        let mut witness = baseline.clone(); witness.target_input.snapshots[0].contracts[0].bid_microdollars += 1; mutants.push(("normalized_input", witness));
        let mut witness = baseline.clone(); witness.target_input.settled_cash_microdollars += 1; mutants.push(("prior_state", witness));
        let mut witness = baseline.clone(); witness.parameters.target_premium_ppb -= 1; mutants.push(("parameters", witness));
        let mut witness = baseline.clone(); witness.statements[0].signature = "A".repeat(88); mutants.push(("ed25519_signature", witness));
        let mut witness = baseline.clone();
        let mut document = hex::decode(&witness.attestation_documents_hex[0]).unwrap();
        *document.last_mut().unwrap() ^= 1;
        witness.attestation_documents_hex[0] = hex::encode(&document);
        witness.statements[0].evidence.attestation_document_sha256 = hex::encode(Sha256::digest(&document));
        mutants.push(("cose_signature", witness));
        let mut encoded_mutants: Vec<_> = mutants.into_iter().map(|(name, witness)| {
            assert!(execute_yolo_target_proof(&witness).is_err(), "native accepted {name}");
            (name, serde_cbor::to_vec(&witness).unwrap())
        }).collect();
        let mut trailing = serde_cbor::to_vec(&baseline).unwrap(); trailing.push(0);
        encoded_mutants.push(("trailing_cbor", trailing));
        encoded_mutants.push(("duplicate_cbor_fields", vec![0xa2, 0x61, b'a', 0, 0x61, b'a', 1]));
        encoded_mutants.push(("oversized_witness_before_allocation", vec![0; MAX_TARGET_WITNESS_BYTES + 1]));
        for (name, encoded) in encoded_mutants {
            let result = client.execute(elf.clone(), stdin(encoded)).await;
            match result {
                Ok((public, report)) => {
                    assert_ne!(report.exit_code, 0, "guest accepted {name}");
                    assert!(public.to_vec().is_empty(), "invalid witness leaked public bytes for {name}");
                }
                Err(_) => {}
            }
            println!("{}", serde_json::json!({"case": name, "valid": false, "rejected": true}));
        }
    });
}
