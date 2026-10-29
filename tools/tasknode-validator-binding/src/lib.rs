use postfiat_crypto_provider::{
    bytes_to_hex, hex_to_bytes, ml_dsa_65_sign_with_context, ml_dsa_65_verify_with_context,
};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const CONTEXT: &[u8] = b"postfiat-l1-v2/tasknode-validator-binding/v1";

// Match the node's ValidatorKeyFile/ValidatorKeyRecord serialization exactly.
// Never derive Debug or include deserialization errors in operator output.
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
struct KeyFile {
    validators: Vec<KeyRecord>,
}
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
struct KeyRecord {
    node_id: String,
    algorithm_id: String,
    public_key_hex: String,
    private_key_hex: String,
}
#[derive(Serialize)]
pub struct Proof {
    pub validator_id: String,
    pub ml_dsa_public_key: String,
    pub signature: String,
}

pub fn validate_challenge(challenge: &str, now: u64) -> Result<(), &'static str> {
    if challenge.len() > 512 {
        return Err("challenge too long");
    }
    let parts: Vec<_> = challenge.split(':').collect();
    if parts.len() != 4
        || parts[0] != "pf-tasknode"
        || !(32..=128).contains(&parts[1].len())
        || !parts[1]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        || parts[2].is_empty()
        || parts[2].len() > 200
        || !parts[2]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("invalid Task Node challenge");
    }
    let expires = parts[3]
        .parse::<u64>()
        .map_err(|_| "invalid challenge expiry")?;
    if expires <= now || expires > now.saturating_add(1860) {
        return Err("challenge expired or exceeds 30-minute lifetime");
    }
    Ok(())
}

fn sign_bytes(raw: &[u8], id: &str, challenge: &str) -> Result<Proof, &'static str> {
    let keys: KeyFile = serde_json::from_slice(raw).map_err(|_| "invalid node key file")?;
    let selected: Vec<_> = keys.validators.iter().filter(|r| r.node_id == id).collect();
    if selected.len() != 1 {
        return Err("node id must match exactly one key record");
    }
    let record = selected[0];
    if record.algorithm_id != "ML-DSA-65" {
        return Err("key algorithm must be ML-DSA-65");
    }
    let public = hex_to_bytes(&record.public_key_hex).map_err(|_| "invalid public key")?;
    let private =
        Zeroizing::new(hex_to_bytes(&record.private_key_hex).map_err(|_| "invalid private key")?);
    let signature = ml_dsa_65_sign_with_context(&private, challenge.as_bytes(), CONTEXT)
        .map_err(|_| "ML-DSA signing failed")?;
    if !ml_dsa_65_verify_with_context(&public, challenge.as_bytes(), &signature, CONTEXT) {
        return Err("node public and private key do not match");
    }
    Ok(Proof {
        validator_id: id.to_owned(),
        ml_dsa_public_key: bytes_to_hex(&public),
        signature: bytes_to_hex(&signature),
    })
}

pub fn sign_file(path: &str, id: &str, challenge: &str, now: u64) -> Result<Proof, &'static str> {
    validate_challenge(challenge, now)?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid validator id");
    }
    let file = std::fs::File::open(path).map_err(|_| "cannot open configured node key file")?;
    let metadata = file
        .metadata()
        .map_err(|_| "cannot inspect node key file")?;
    if !metadata.is_file() || metadata.mode() & 0o077 != 0 || metadata.len() > 1_048_576 {
        return Err("node key file must be a private regular file (mode 600 or 400)");
    }
    let mut raw = Zeroizing::new(Vec::new());
    file.take(1_048_577)
        .read_to_end(&mut raw)
        .map_err(|_| "cannot read node key file")?;
    if raw.len() > 1_048_576 {
        return Err("node key file too large");
    }
    sign_bytes(&raw, id, challenge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::ml_dsa_65_keygen_from_seed;
    fn fixture() -> (serde_json::Value, String) {
        let k = ml_dsa_65_keygen_from_seed(&[7; 32]);
        (
            serde_json::json!({"validators":[{"node_id":"validator-test", "algorithm_id":"ML-DSA-65", "public_key_hex":bytes_to_hex(&k.public_key), "private_key_hex":bytes_to_hex(&k.private_key)}]}),
            "pf-tasknode:0123456789abcdef0123456789abcdef:acct_test:1793199600".to_owned(),
        )
    }
    #[test]
    fn challenge_domain_and_expiry() {
        let (_, c) = fixture();
        assert!(validate_challenge(&c, 1793198000).is_ok());
        for x in [
            c.replace("pf-tasknode", "transfer"),
            c.replace("0123456789abcdef0123456789abcdef", "short"),
            c.replace("acct_test", "acct/other"),
        ] {
            assert!(validate_challenge(&x, 1793198000).is_err());
        }
        assert!(validate_challenge(&c, 1793199600).is_err());
        assert!(validate_challenge(&c, 1793197000).is_err());
    }
    #[test]
    fn proof_only_public_and_domain_separated() {
        let (f, c) = fixture();
        let p = sign_bytes(&serde_json::to_vec(&f).unwrap(), "validator-test", &c).unwrap();
        let j = serde_json::to_value(&p).unwrap();
        assert_eq!(j.as_object().unwrap().len(), 3);
        let pk = hex_to_bytes(&p.ml_dsa_public_key).unwrap();
        let sig = hex_to_bytes(&p.signature).unwrap();
        assert_eq!(pk.len(), 1952);
        assert_eq!(sig.len(), 3309);
        assert!(ml_dsa_65_verify_with_context(
            &pk,
            c.as_bytes(),
            &sig,
            CONTEXT
        ));
        assert!(!ml_dsa_65_verify_with_context(
            &pk,
            c.as_bytes(),
            &sig,
            b"postfiat-l1-v2/tx/v1"
        ));
        assert!(!ml_dsa_65_verify_with_context(
            &pk,
            b"different message",
            &sig,
            CONTEXT
        ));
    }
    #[test]
    fn reject_mismatch_duplicate_unknown_and_no_secret_errors() {
        let (mut f, c) = fixture();
        assert!(sign_bytes(&serde_json::to_vec(&f).unwrap(), "unknown", &c).is_err());
        let other = ml_dsa_65_keygen_from_seed(&[8; 32]);
        f["validators"][0]["public_key_hex"] = bytes_to_hex(&other.public_key).into();
        assert_eq!(
            sign_bytes(&serde_json::to_vec(&f).unwrap(), "validator-test", &c).err(),
            Some("node public and private key do not match")
        );
        let (mut f, c) = fixture();
        let duplicate = f["validators"][0].clone();
        f["validators"].as_array_mut().unwrap().push(duplicate);
        assert!(sign_bytes(&serde_json::to_vec(&f).unwrap(), "validator-test", &c).is_err());
        assert_eq!(
            sign_bytes(b"SECRET malformed", "validator-test", &c).err(),
            Some("invalid node key file")
        );
    }
}
