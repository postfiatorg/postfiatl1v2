use k256::ecdsa::signature::{RandomizedSigner as _, Signer as _, Verifier as _};
use k256::ecdsa::{
    Signature as EcdsaSignature, SigningKey as EcdsaSigningKey, VerifyingKey as EcdsaVerifyingKey,
};
use k256::schnorr::{
    Signature as Bip340Signature, SigningKey as Bip340SigningKey,
    VerifyingKey as Bip340VerifyingKey,
};

use crate::CryptoError;

/// Verify a BIP-340 Schnorr signature over `SHA-256(message)` for a 32-byte x-only
/// secp256k1 public key (used exclusively for Pillar 6 Nostr identity-binding proofs).
pub fn bip340_verify(xonly_pubkey: &[u8], message: &[u8], signature: &[u8]) -> bool {
    if xonly_pubkey.len() != 32 || signature.len() != 64 {
        return false;
    }
    let Ok(vk) = Bip340VerifyingKey::from_bytes(xonly_pubkey) else {
        return false;
    };
    let Ok(sig) = Bip340Signature::try_from(signature) else {
        return false;
    };
    vk.verify(message, &sig).is_ok()
}

/// Deterministically sign `SHA-256(message)` with BIP-340 Schnorr using zero auxiliary randomness.
pub fn bip340_sign_deterministic(
    secret_key: &[u8; 32],
    message: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let sk = Bip340SigningKey::from_bytes(secret_key)
        .map_err(|err| CryptoError::new(format!("invalid secp256k1 secret key: {err}")))?;
    let sig = sk
        .try_sign_with_rng(&mut ZeroRng, message)
        .map_err(|err| CryptoError::new(format!("bip340 signing failed: {err}")))?;
    Ok(sig.to_bytes().to_vec())
}

/// Derive the 32-byte x-only BIP-340 public key from a 32-byte secp256k1 secret key.
pub fn bip340_xonly_pubkey_from_secret(secret_key: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    let sk = Bip340SigningKey::from_bytes(secret_key)
        .map_err(|err| CryptoError::new(format!("invalid secp256k1 secret key: {err}")))?;
    Ok(sk.verifying_key().to_bytes().to_vec())
}

/// Verify a strict low-S DER-encoded secp256k1 ECDSA signature over `SHA-256(message)`.
/// High-S signatures and non-canonical DER encodings are strictly rejected.
pub fn secp256k1_ecdsa_verify_low_s_der(
    pubkey_bytes: &[u8],
    message: &[u8],
    der_signature: &[u8],
) -> bool {
    let Ok(vk) = EcdsaVerifyingKey::from_sec1_bytes(pubkey_bytes) else {
        return false;
    };
    let Ok(sig) = EcdsaSignature::from_der(der_signature) else {
        return false;
    };
    // Enforce strict low-S malleability protection: normalize_s() returns Some(_) iff S is high.
    if sig.normalize_s().is_some() {
        return false;
    }
    // Enforce canonical DER round-trip (rejects trailing junk or non-minimal DER integers).
    if sig.to_der().as_bytes() != der_signature {
        return false;
    }
    vk.verify(message, &sig).is_ok()
}

/// Produce a canonical low-S DER-encoded secp256k1 ECDSA signature over `SHA-256(message)`.
pub fn secp256k1_ecdsa_sign_low_s_der(
    secret_key: &[u8; 32],
    message: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let sk = EcdsaSigningKey::from_slice(secret_key)
        .map_err(|err| CryptoError::new(format!("invalid secp256k1 secret key: {err}")))?;
    let sig: EcdsaSignature = sk.sign(message);
    let low_s = sig.normalize_s().unwrap_or(sig);
    Ok(low_s.to_der().as_bytes().to_vec())
}

/// Derive the 33-byte compressed SEC1 secp256k1 public key from a 32-byte secret key.
pub fn secp256k1_compressed_pubkey_from_secret(
    secret_key: &[u8; 32],
) -> Result<Vec<u8>, CryptoError> {
    let sk = EcdsaSigningKey::from_slice(secret_key)
        .map_err(|err| CryptoError::new(format!("invalid secp256k1 secret key: {err}")))?;
    Ok(sk
        .verifying_key()
        .to_encoded_point(true)
        .as_bytes()
        .to_vec())
}

struct ZeroRng;

impl k256::elliptic_curve::rand_core::RngCore for ZeroRng {
    fn next_u32(&mut self) -> u32 {
        0
    }
    fn next_u64(&mut self) -> u64 {
        0
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        dest.fill(0);
    }
    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> Result<(), k256::elliptic_curve::rand_core::Error> {
        dest.fill(0);
        Ok(())
    }
}

impl k256::elliptic_curve::rand_core::CryptoRng for ZeroRng {}

#[cfg(test)]
mod classical_tests {
    use super::*;

    #[test]
    fn bip340_sign_and_verify_roundtrip() {
        let sk = [0x11u8; 32];
        let pk = bip340_xonly_pubkey_from_secret(&sk).expect("xonly pk");
        let msg = b"postfiat-l1-v2/identity-nostr/v1\nop=mint\n";
        let sig = bip340_sign_deterministic(&sk, msg).expect("sign");
        assert!(bip340_verify(&pk, msg, &sig));
        assert!(!bip340_verify(&pk, b"wrong", &sig));
        let mut bad = sig.clone();
        bad[0] ^= 1;
        assert!(!bip340_verify(&pk, msg, &bad));
    }

    #[test]
    fn secp256k1_low_s_der_roundtrip_and_rejection() {
        let sk = [0x22u8; 32];
        let pk = secp256k1_compressed_pubkey_from_secret(&sk).expect("compressed pk");
        let msg = b"postfiat-l1-v2/identity-old-chain/v1\n";
        let der = secp256k1_ecdsa_sign_low_s_der(&sk, msg).expect("sign");
        assert!(secp256k1_ecdsa_verify_low_s_der(&pk, msg, &der));
        assert!(!secp256k1_ecdsa_verify_low_s_der(&pk, b"other", &der));
        let mut padded = der.clone();
        padded.push(0x00);
        assert!(!secp256k1_ecdsa_verify_low_s_der(&pk, msg, &padded));
    }
}
