use std::time::Instant;

use postfiat_crypto_provider::{
    ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, ml_dsa_65_verify_with_context,
    BLOCK_CERTIFICATE_SIGNATURE_CONTEXT, ML_DSA_65_PUBLIC_KEY_BYTES, ML_DSA_65_SIGNATURE_BYTES,
};

fn quorum(n: usize) -> usize {
    (2 * n / 3) + 1
}

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000);

    let key_pair = ml_dsa_65_keygen_from_seed(&[11u8; 32]);
    let message = b"postfiat-l1-v2/ml-dsa-budget/v1/block-certificate";
    let signature = ml_dsa_65_sign_with_context_seed(
        &key_pair.private_key,
        message,
        BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
        &[12u8; 32],
    )
    .expect("sign benchmark message");

    assert_eq!(signature.len(), ML_DSA_65_SIGNATURE_BYTES);
    assert_eq!(key_pair.public_key.len(), ML_DSA_65_PUBLIC_KEY_BYTES);
    assert!(ml_dsa_65_verify_with_context(
        &key_pair.public_key,
        message,
        &signature,
        BLOCK_CERTIFICATE_SIGNATURE_CONTEXT
    ));

    let started = Instant::now();
    for _ in 0..iterations {
        assert!(ml_dsa_65_verify_with_context(
            &key_pair.public_key,
            message,
            &signature,
            BLOCK_CERTIFICATE_SIGNATURE_CONTEXT
        ));
    }
    let elapsed = started.elapsed();
    let elapsed_seconds = elapsed.as_secs_f64();
    let verifies_per_second = iterations as f64 / elapsed_seconds;
    let micros_per_verify = (elapsed_seconds * 1_000_000.0) / iterations as f64;

    println!("iterations={iterations}");
    println!("public_key_bytes={ML_DSA_65_PUBLIC_KEY_BYTES}");
    println!("signature_bytes={ML_DSA_65_SIGNATURE_BYTES}");
    println!("elapsed_seconds={elapsed_seconds:.6}");
    println!("verifies_per_second={verifies_per_second:.2}");
    println!("micros_per_verify={micros_per_verify:.2}");

    for validators in [5usize, 35, 100] {
        let q = quorum(validators);
        let signature_payload_bytes = q * ML_DSA_65_SIGNATURE_BYTES;
        let validator_id_bytes = q * 32;
        let total_detached_bytes = signature_payload_bytes + validator_id_bytes;
        let serialized_verify_ms = micros_per_verify * q as f64 / 1000.0;
        println!(
            "cert_budget validators={validators} quorum={q} signature_payload_bytes={signature_payload_bytes} validator_id_bytes={validator_id_bytes} total_detached_bytes={total_detached_bytes} serialized_verify_ms={serialized_verify_ms:.3}"
        );
    }
}
