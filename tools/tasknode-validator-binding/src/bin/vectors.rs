use postfiat_crypto_provider::{
    bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed,
};
use tasknode_l1_validator_signer::CONTEXT;
fn main() {
    let mut vectors = Vec::new();
    for i in 1u8..=3 {
        let key = ml_dsa_65_keygen_from_seed(&[i; 32]);
        let message = format!("pf-tasknode:{:032x}:acct_vector_{i}:1793199600", i);
        let signature = ml_dsa_65_sign_with_context_seed(
            &key.private_key,
            message.as_bytes(),
            CONTEXT,
            &[i + 10; 32],
        )
        .unwrap();
        vectors.push(serde_json::json!({"name":format!("valid-{i}"),"ml_dsa_public_key":bytes_to_hex(&key.public_key),"message":message,"context":String::from_utf8_lossy(CONTEXT),"signature":bytes_to_hex(&signature),"expected_valid":true}));
    }
    let wrong_context_key = ml_dsa_65_keygen_from_seed(&[1; 32]);
    let wrong_context_signature = ml_dsa_65_sign_with_context_seed(
        &wrong_context_key.private_key,
        vectors[0]["message"].as_str().unwrap().as_bytes(),
        b"postfiat-l1-v2/tx/v1",
        &[11; 32],
    )
    .unwrap();
    for (name, field, value) in [
        (
            "wrong-key",
            "ml_dsa_public_key",
            vectors[1]["ml_dsa_public_key"].clone(),
        ),
        (
            "wrong-context",
            "signature",
            serde_json::json!(bytes_to_hex(&wrong_context_signature)),
        ),
        (
            "wrong-message",
            "message",
            serde_json::json!("pf-tasknode:00000000000000000000000000000001:acct_other:1793199600"),
        ),
        ("truncated-signature", "signature", serde_json::json!("00")),
    ] {
        let mut v = vectors[0].clone();
        v["name"] = name.into();
        v[field] = value;
        v["expected_valid"] = false.into();
        if name == "wrong-context" {
            v["signed_context"] = "postfiat-l1-v2/tx/v1".into();
        }
        vectors.push(v);
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"schema":"postfiat-tasknode-l1-vectors-v1","algorithm":"ML-DSA-65","implementation":"postfiatl1v2/crates/crypto_provider","public_key_bytes":1952,"signature_bytes":3309,"vectors":vectors})).unwrap());
}
