use std::{env, fs, process::exit};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!(
            "usage: {} <proof-calldata.bin> <public-values.bin> <0xvkey>",
            args[0]
        );
        exit(2);
    }
    let proof = fs::read(&args[1]).expect("read proof-calldata.bin");
    let public_values = fs::read(&args[2]).expect("read public-values.bin");
    let vkey = &args[3];
    // Same limits as the governed Ethereum route profile (max_proof_bytes / max_public_values_bytes 4096).
    let result = postfiat_execution::verify_bounded_sp1_groth16_with_config(
        postfiat_types::NAV_PROFILE_VERIFIER_SP1_GROTH16,
        postfiat_types::NAV_PROFILE_VERIFIER_SP1_GROTH16,
        vkey,
        4096,
        4096,
        &proof,
        &public_values,
    );
    if let Err(error) = result {
        eprintln!(
            "{}",
            serde_json::json!({"verified": false, "code": error.code(), "message": error.message()})
        );
        exit(1);
    }
    let decoded = match postfiat_types::PfUsdcEthereumIngressPublicValuesV1::from_canonical_bytes(
        &public_values,
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::json!({"verified": true, "public_values_decoded": false, "error": error})
            );
            exit(1);
        }
    };
    println!(
        "{}",
        serde_json::json!({
            "verified": true,
            "verifier": "postfiat_execution::verify_bounded_sp1_groth16_with_config (sp1-verifier Groth16Verifier + GROTH16_VK_BYTES)",
            "program_vkey": vkey,
            "proof_bytes": proof.len(),
            "public_values_bytes": public_values.len(),
            "public_values": serde_json::to_value(&decoded).expect("serialize public values"),
        })
    );
}
