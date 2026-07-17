use fips204::ml_dsa_65;
use fips204::traits::{KeyGen, SerDes, Signer, Verifier};
use sha3::{Digest, Sha3_384};
use zeroize::Zeroizing;

pub const CRATE_PURPOSE: &str = "versioned post-quantum crypto provider interfaces";
pub const ML_DSA_65_ALGORITHM: &str = "ML-DSA-65";
pub const ML_DSA_65_PUBLIC_KEY_BYTES: usize = ml_dsa_65::PK_LEN;
pub const ML_DSA_65_SIGNATURE_BYTES: usize = ml_dsa_65::SIG_LEN;
pub const TX_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/tx/v1";
pub const BLOCK_CERTIFICATE_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/block-certificate/v1";
pub const BRIDGE_WITNESS_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/bridge-witness/v1";
pub const ADMISSION_RECEIPT_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/admission-receipt/v1";

#[derive(Debug)]
pub struct MlDsa65KeyPair {
    pub public_key: Vec<u8>,
    pub private_key: Zeroizing<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CryptoError {
    message: String,
}

impl CryptoError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CryptoError {}

pub fn ml_dsa_65_keygen() -> Result<MlDsa65KeyPair, CryptoError> {
    let (public_key, private_key) =
        ml_dsa_65::KG::try_keygen().map_err(|error| CryptoError::new(error.to_string()))?;
    Ok(MlDsa65KeyPair {
        public_key: public_key.into_bytes().to_vec(),
        private_key: Zeroizing::new(private_key.into_bytes().to_vec()),
    })
}

pub fn ml_dsa_65_keygen_from_seed(seed: &[u8; 32]) -> MlDsa65KeyPair {
    let (public_key, private_key) = ml_dsa_65::KG::keygen_from_seed(seed);
    MlDsa65KeyPair {
        public_key: public_key.into_bytes().to_vec(),
        private_key: Zeroizing::new(private_key.into_bytes().to_vec()),
    }
}

pub fn ml_dsa_65_sign(private_key: &[u8], message: &[u8]) -> Result<Vec<u8>, CryptoError> {
    ml_dsa_65_sign_with_context(private_key, message, TX_SIGNATURE_CONTEXT)
}

pub fn ml_dsa_65_sign_with_context(
    private_key: &[u8],
    message: &[u8],
    context: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let private_key_bytes = Zeroizing::new(private_key_array(private_key)?);
    let private_key =
        ml_dsa_65::PrivateKey::try_from_bytes(*private_key_bytes).map_err(CryptoError::new)?;
    let signature = private_key
        .try_sign(message, context)
        .map_err(CryptoError::new)?;
    Ok(signature.to_vec())
}

pub fn ml_dsa_65_sign_with_context_seed(
    private_key: &[u8],
    message: &[u8],
    context: &[u8],
    seed: &[u8; 32],
) -> Result<Vec<u8>, CryptoError> {
    let private_key_bytes = Zeroizing::new(private_key_array(private_key)?);
    let private_key =
        ml_dsa_65::PrivateKey::try_from_bytes(*private_key_bytes).map_err(CryptoError::new)?;
    let signature = private_key
        .try_sign_with_seed(seed, message, context)
        .map_err(CryptoError::new)?;
    Ok(signature.to_vec())
}

pub fn ml_dsa_65_verify(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    ml_dsa_65_verify_with_context(public_key, message, signature, TX_SIGNATURE_CONTEXT)
}

pub fn ml_dsa_65_verify_with_context(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    context: &[u8],
) -> bool {
    let Ok(public_key) = public_key_array(public_key) else {
        return false;
    };
    let Ok(signature) = signature_array(signature) else {
        return false;
    };
    let Ok(public_key) = ml_dsa_65::PublicKey::try_from_bytes(public_key) else {
        return false;
    };
    public_key.verify(message, &signature, context)
}

pub fn ml_dsa_65_validate_public_key(public_key: &[u8]) -> Result<(), CryptoError> {
    let public_key = public_key_array(public_key)?;
    ml_dsa_65::PublicKey::try_from_bytes(public_key).map_err(CryptoError::new)?;
    Ok(())
}

pub fn address_from_public_key(public_key: &[u8]) -> String {
    let digest = hash_bytes("postfiat.address.v1", public_key);
    format!("pf{}", bytes_to_hex(&digest[..20]))
}

pub fn hash_hex(domain: &str, bytes: &[u8]) -> String {
    bytes_to_hex(&hash_bytes(domain, bytes))
}

pub fn hash_bytes(domain: &str, bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha3_384::new();
    hasher.update(domain.as_bytes());
    hasher.update([0u8]);
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, CryptoError> {
    if !hex.len().is_multiple_of(2) {
        return Err(CryptoError::new("hex string has odd length"));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> Result<u8, CryptoError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(CryptoError::new("invalid hex digit")),
    }
}

fn private_key_array(private_key: &[u8]) -> Result<[u8; ml_dsa_65::SK_LEN], CryptoError> {
    private_key
        .try_into()
        .map_err(|_| CryptoError::new("invalid ML-DSA-65 private key length"))
}

fn public_key_array(public_key: &[u8]) -> Result<[u8; ml_dsa_65::PK_LEN], CryptoError> {
    public_key
        .try_into()
        .map_err(|_| CryptoError::new("invalid ML-DSA-65 public key length"))
}

fn signature_array(signature: &[u8]) -> Result<[u8; ml_dsa_65::SIG_LEN], CryptoError> {
    signature
        .try_into()
        .map_err(|_| CryptoError::new("invalid ML-DSA-65 signature length"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ml_dsa_65_signs_and_verifies() {
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let message = b"postfiat test transaction";
        let signature = ml_dsa_65_sign(&key_pair.private_key, message).expect("sign");

        assert!(ml_dsa_65_verify(&key_pair.public_key, message, &signature));
        ml_dsa_65_validate_public_key(&key_pair.public_key).expect("valid public key");
        assert!(ml_dsa_65_validate_public_key(&key_pair.public_key[..32]).is_err());
        assert!(!ml_dsa_65_verify(
            &key_pair.public_key,
            b"tampered",
            &signature
        ));
        let certificate_signature = ml_dsa_65_sign_with_context(
            &key_pair.private_key,
            message,
            BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
        )
        .expect("block certificate sign");
        assert!(ml_dsa_65_verify_with_context(
            &key_pair.public_key,
            message,
            &certificate_signature,
            BLOCK_CERTIFICATE_SIGNATURE_CONTEXT
        ));
        assert!(!ml_dsa_65_verify(
            &key_pair.public_key,
            message,
            &certificate_signature
        ));
        let seed = [7u8; 32];
        let deterministic_signature = ml_dsa_65_sign_with_context_seed(
            &key_pair.private_key,
            message,
            BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
            &seed,
        )
        .expect("seeded sign");
        assert_eq!(
            deterministic_signature,
            ml_dsa_65_sign_with_context_seed(
                &key_pair.private_key,
                message,
                BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
                &seed
            )
            .expect("seeded sign repeat")
        );
        assert!(ml_dsa_65_verify_with_context(
            &key_pair.public_key,
            message,
            &deterministic_signature,
            BLOCK_CERTIFICATE_SIGNATURE_CONTEXT
        ));
    }

    #[test]
    fn ml_dsa_65_seeded_keygen_is_reproducible() {
        let seed = [42u8; 32];
        let first = ml_dsa_65_keygen_from_seed(&seed);
        let second = ml_dsa_65_keygen_from_seed(&seed);
        let other = ml_dsa_65_keygen_from_seed(&[43u8; 32]);

        assert_eq!(first.public_key, second.public_key);
        assert_eq!(first.private_key.as_slice(), second.private_key.as_slice());
        assert_ne!(first.public_key, other.public_key);
        assert_ne!(first.private_key.as_slice(), other.private_key.as_slice());
        assert_eq!(first.public_key.len(), ml_dsa_65::PK_LEN);
        assert_eq!(first.private_key.len(), ml_dsa_65::SK_LEN);
        assert_eq!(address_from_public_key(&first.public_key).len(), 42);

        let signature_seed = [9u8; 32];
        let message = b"postfiat deterministic crypto fixture";
        let signature = ml_dsa_65_sign_with_context_seed(
            &first.private_key,
            message,
            TX_SIGNATURE_CONTEXT,
            &signature_seed,
        )
        .expect("seeded signature");

        assert_eq!(
            signature,
            ml_dsa_65_sign_with_context_seed(
                &second.private_key,
                message,
                TX_SIGNATURE_CONTEXT,
                &signature_seed
            )
            .expect("repeat seeded signature")
        );
        assert!(ml_dsa_65_verify(&first.public_key, message, &signature));
        assert!(!ml_dsa_65_verify(&other.public_key, message, &signature));
    }

    #[test]
    fn hex_round_trip() {
        let bytes = [0u8, 1, 2, 10, 15, 16, 255];
        let encoded = bytes_to_hex(&bytes);
        let decoded = hex_to_bytes(&encoded).expect("decode");
        assert_eq!(decoded, bytes);
    }
}
