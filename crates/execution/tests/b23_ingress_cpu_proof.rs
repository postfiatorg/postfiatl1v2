use postfiat_execution::verify_bounded_sp1_groth16_with_config;
use postfiat_types::{PfUsdcEthereumIngressPublicValuesV1, NAV_PROFILE_VERIFIER_SP1_GROTH16};
use std::{env, fs, path::PathBuf};

const VKEY: &str = "0x00d8cbf06d5640adde0fef863b3c689c4ab3442b0dc91d28ac4cc5f9c9b2ccbb";
const ELF_SHA256: &str = "b6a06cb90ee81477a33c9a5902458db5b379cc40b765988cebb0ec46d08766fe";

/// Opt-in, read-only verification of an operator's real CPU proof. No submission.
#[test]
#[ignore = "requires B23_INGRESS_PROOF_DIR containing a real rev-2 CPU Groth16 proof"]
fn real_cpu_ingress_proof_passes_node_verifier() {
    let directory = PathBuf::from(env::var("B23_INGRESS_PROOF_DIR").unwrap());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("proof-report.json")).unwrap()).unwrap();
    assert_eq!(report["program_vkey"], VKEY);
    assert_eq!(report["elf_sha256"], ELF_SHA256);
    assert_eq!(report["prover_backend"], "cpu");
    assert_eq!(report["host_execute_skipped"], false);
    let proof = fs::read(directory.join("proof-calldata.bin")).unwrap();
    let public = fs::read(directory.join("public-values.bin")).unwrap();
    let verify = |key: &str, values: &[u8]| {
        verify_bounded_sp1_groth16_with_config(
            NAV_PROFILE_VERIFIER_SP1_GROTH16,
            NAV_PROFILE_VERIFIER_SP1_GROTH16,
            key,
            0,
            0,
            &proof,
            values,
        )
    };
    verify(VKEY, &public).expect("real proof must pass the node's execution verifier");
    let values = PfUsdcEthereumIngressPublicValuesV1::from_canonical_bytes(&public).unwrap();
    assert_eq!(values.vault_address, "0xe7a432a28b20621a70e845c7f202004cf98e7a2c");
    assert_eq!(values.source_chain_id, 1);
    let mut tampered = public.clone();
    tampered[0] ^= 1;
    assert!(verify(VKEY, &tampered).is_err());
    assert!(verify(&format!("0x{}", "00".repeat(32)), &public).is_err());
    println!("{}", serde_json::json!({
        "node_verifier_accepted": true,
        "program_vkey": VKEY,
        "elf_sha256": ELF_SHA256,
        "proof_bytes": proof.len(),
        "public_values_bytes": public.len(),
        "source_proof_hash": postfiat_types::pfusdc_ingress_proof_hash_v1(&proof),
        "source_public_values_hash": postfiat_types::pfusdc_ingress_public_values_hash_v1(&public),
        "deposit_id": values.deposit_id,
        "amount_atoms": values.amount_atoms,
        "route_epoch": values.route_epoch,
        "tampered_public_values_rejected": true,
        "wrong_vkey_rejected": true
    }));
}
