//! Writes the fixed Rust-signed native-session open request checked by
//! wallet-proxy/test_native_session.js:
//! `cargo run -q -p postfiat-crypto-provider --example native_session_vector > wallet-proxy/fixtures/native-session-rust-vector.json`
use postfiat_crypto_provider::{
    bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed,
    ml_dsa_65_verify_with_context,
};

const CONTEXT: &[u8] = b"postfiat-l1-v2/native-session/v1";
const SCHEMA: &str = "postfiat-native-session-open-v1";
const ORIGIN: &str = "https://gateway.example.test";
const CHAIN_ID: &str = "postfiat-wan-devnet-2";
const GENESIS_HASH_HEX: &str = "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9";
const ISSUED_AT_MS: u64 = 1_793_000_000_000;

fn main() {
    // Fixed test-only seeds: the vector is reproducible and holds no real key.
    let key = ml_dsa_65_keygen_from_seed(&[0x4e; 32]);
    let public_key_hex = bytes_to_hex(&key.public_key);
    let client_nonce_hex = bytes_to_hex(&[0x6e; 32]);
    // The signed bytes: newline-joined, as wallet-proxy/native-session.js openMessage().
    let message = [
        SCHEMA,
        ORIGIN,
        CHAIN_ID,
        GENESIS_HASH_HEX,
        &public_key_hex,
        &client_nonce_hex,
        &ISSUED_AT_MS.to_string(),
    ]
    .join("\n");
    let signature = ml_dsa_65_sign_with_context_seed(
        &key.private_key,
        message.as_bytes(),
        CONTEXT,
        &[0x53; 32],
    )
    .expect("sign native session vector");
    assert!(ml_dsa_65_verify_with_context(
        &key.public_key,
        message.as_bytes(),
        &signature,
        CONTEXT
    ));
    let vector = serde_json::json!({
        "schema": "postfiat-native-session-rust-vector-v1",
        "generator": "crates/crypto_provider/examples/native_session_vector.rs",
        "context": std::str::from_utf8(CONTEXT).expect("ascii context"),
        "request": {
            "schema": SCHEMA,
            "algorithm_id": "ML-DSA-65",
            "public_key_hex": public_key_hex,
            "client_nonce_hex": client_nonce_hex,
            "origin": ORIGIN,
            "chain_id": CHAIN_ID,
            "genesis_hash_hex": GENESIS_HASH_HEX,
            "issued_at_ms": ISSUED_AT_MS,
            "signature_hex": bytes_to_hex(&signature),
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&vector).expect("vector json")
    );
}
