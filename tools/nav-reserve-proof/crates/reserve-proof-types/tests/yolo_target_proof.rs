use reserve_proof_types::yolo_target_proof::execute_yolo_target_proof;
use reserve_proof_types::yolo_witness::{
    decode_target_witness, TargetAcceptanceV1, TargetProofWitnessV1,
};

fn fixtures() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/yolo_target_proof_v1");
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    paths
}

fn fixture(name: &str) -> (TargetProofWitnessV1, Vec<u8>) {
    let value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            fixtures()
                .into_iter()
                .find(|path| path.file_stem().unwrap() == name)
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    (
        serde_json::from_value(value["witness"].clone()).unwrap(),
        hex::decode(value["expectedPublicValuesHex"].as_str().unwrap()).unwrap(),
    )
}

#[test]
fn all_fourteen_signed_witnesses_match_python_public_bytes() {
    assert_eq!(fixtures().len(), 14);
    for path in fixtures() {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let witness: TargetProofWitnessV1 =
            serde_json::from_value(value["witness"].clone()).unwrap();
        let cbor = serde_cbor::to_vec(&witness).unwrap();
        let decoded = decode_target_witness(&cbor).unwrap();
        assert_eq!(witness, decoded);
        let public = execute_yolo_target_proof(&decoded)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let bytes = public.encode().unwrap();
        assert_eq!(
            hex::encode(&bytes),
            value["expectedPublicValuesHex"].as_str().unwrap(),
            "{}",
            path.display()
        );
        assert_eq!(bytes.len(), 408);
        let expected = TargetAcceptanceV1 {
            program_sha256: witness.program_sha256.clone(),
            collection_manifest_sha256: witness.manifest.sha256().unwrap(),
        };
        expected.verify(&public).unwrap();
        assert!(!bytes.windows(4).any(|part| part == b"HOOD"));
        let document = hex::decode(&witness.attestation_documents_hex[0]).unwrap();
        assert!(document.len() > bytes.len());
    }
}

#[test]
fn rejects_witness_substitution_across_every_trust_boundary() {
    let (original, _) = fixture("baseline");
    let mut mutants = Vec::new();
    let mut value = original.clone();
    value.root_certificate_der_hex.replace_range(0..2, "00");
    mutants.push(("root", value));
    let mut value = original.clone();
    value.manifest.proof_policy.approved_pcr_sets[0][0].sha384 = "22".repeat(48);
    mutants.push(("PCR policy", value));
    let mut value = original.clone();
    value.manifest.proof_policy.max_age_ms = 1;
    mutants.push(("time policy", value));
    let mut value = original.clone();
    value.manifest.epoch.window_end_utc = "2026-09-04T15:00:01Z".into();
    mutants.push(("epoch time", value));
    let mut value = original.clone();
    value.statements.swap(0, 1);
    mutants.push(("statement order", value));
    let mut value = original.clone();
    value.statements[1].previous_statement_sha256 = Some("11".repeat(32));
    mutants.push(("statement chain", value));
    let mut value = original.clone();
    value.statements[0].evidence.statement_key = "12".repeat(32);
    mutants.push(("statement key", value));
    let mut value = original.clone();
    value.statements[0].signature = "A".repeat(88);
    mutants.push(("Ed25519 signature", value));
    let mut value = original.clone();
    value.statements[0].evidence.debug_mode = true;
    mutants.push(("debug", value));
    let mut value = original.clone();
    value.commitments[0].contracts_sha256 = "13".repeat(32);
    mutants.push(("collection", value));
    let mut value = original.clone();
    value.target_input.snapshots[0].contracts[0].bid_microdollars += 1;
    mutants.push(("normalized quote", value));
    let mut value = original.clone();
    value.target_input.settled_cash_microdollars += 1;
    mutants.push(("prior state", value));
    let mut value = original.clone();
    value.parameters.target_premium_ppb -= 1;
    mutants.push(("parameters", value));
    let mut value = original.clone();
    value.target_input.methodology_sha256 = "14".repeat(32);
    mutants.push(("methodology", value));
    let mut value = original.clone();
    value.manifest.run_id.push('x');
    mutants.push(("run", value));
    let mut value = original.clone();
    value.manifest.normalization_sources_sha256 = "15".repeat(32);
    mutants.push(("normalization sources", value));
    let mut value = original.clone();
    value.attestation_documents_hex[0].push_str("00");
    mutants.push(("document", value));
    for (name, value) in mutants {
        assert!(
            execute_yolo_target_proof(&value).is_err(),
            "accepted substituted {name}"
        );
    }
}

#[test]
fn rejects_self_selected_public_policy_and_program_even_with_valid_native_claims() {
    let (original, _) = fixture("baseline");
    let expected = TargetAcceptanceV1 {
        program_sha256: original.program_sha256.clone(),
        collection_manifest_sha256: original.manifest.sha256().unwrap(),
    };
    let (another, _) = fixture("incumbent_30");
    // This is a valid signed run under a different synthetic root/manifest.
    assert_ne!(
        another.manifest.proof_policy.root_certificate_sha256,
        original.manifest.proof_policy.root_certificate_sha256
    );
    let public = execute_yolo_target_proof(&another).unwrap();
    assert!(expected.verify(&public).is_err());
    let mut alternate_program = original;
    alternate_program.program_sha256 = "ef".repeat(32);
    let public = execute_yolo_target_proof(&alternate_program).unwrap();
    assert!(expected.verify(&public).is_err());
}

#[test]
fn acyclic_statement_hashes_match_python_fixture_and_bind_document() {
    let (value, _) = fixture("baseline");
    let statement = &value.statements[0];
    let mut changed = statement.clone();
    changed.evidence.attestation_document_sha256 = "00".repeat(32);
    assert_eq!(
        changed.attestation_binding_digest().unwrap(),
        statement.attestation_binding_digest().unwrap()
    );
    assert_ne!(
        changed.signing_digest().unwrap(),
        statement.signing_digest().unwrap()
    );
    assert_ne!(changed.sha256().unwrap(), statement.sha256().unwrap());
}
