use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Nonce,
};
use ff::Field;
use group::{prime::PrimeCurveAffine, Curve, GroupEncoding};
use pasta_curves::pallas;
use rand::{rngs::OsRng, CryptoRng, RngCore};
use sha3::{Digest, Sha3_256};
use zeroize::{DefaultIsZeroes, Zeroizing};

use crate::asset_orchard::{
    asset_orchard_incoming_viewing_key_from_seed, asset_orchard_note_for_address,
    build_asset_orchard_wallet_note_with_rho,
    validate_asset_orchard_wallet_note_for_pool, AssetOrchardBoundedBytes, AssetOrchardError,
    AssetOrchardFieldElement, AssetOrchardShieldedAddress, AssetOrchardShieldedKeys,
    AssetOrchardSwapAction, AssetOrchardWalletNote, AssetTag,
    ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES, ASSET_ORCHARD_RSEED_BYTES,
};

pub const ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC: &[u8; 8] = b"PFAOENC1";
const EPK_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const TAG_BYTES: usize = 16;
const HEADER_BYTES: usize = ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC.len() + EPK_BYTES + NONCE_BYTES;
const KDF_DOMAIN: &[u8] = b"postfiat.asset_orchard.note_encryption.kdf.v1";
const AAD_DOMAIN: &[u8] = b"postfiat.asset_orchard.note_encryption.aad.v1";

/// pasta_curves 0.5 implements no `Zeroize` for its scalars or points, but `pallas::Scalar`,
/// `pallas::Base` and `pallas::Affine` are `Copy + Default` with all-zero defaults (zero, identity),
/// so this owner lets `Zeroizing` overwrite them on drop. Copies made inside curve arithmetic
/// (projective points, registers) are outside its reach.
#[derive(Clone, Copy, Default)]
struct SecretCurveValue<T: Copy + Default>(T);

impl<T: Copy + Default> DefaultIsZeroes for SecretCurveValue<T> {}

fn secret_curve_value<T: Copy + Default>(value: T) -> Zeroizing<SecretCurveValue<T>> {
    Zeroizing::new(SecretCurveValue(value))
}

pub fn encrypt_asset_orchard_wallet_note(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    note: &AssetOrchardWalletNote,
) -> Result<AssetOrchardBoundedBytes, AssetOrchardError> {
    encrypt_asset_orchard_wallet_note_with_rng(
        chain_id,
        genesis_hash,
        protocol_version,
        note,
        &mut OsRng,
    )
}

fn encrypt_asset_orchard_wallet_note_with_rng<R: RngCore + CryptoRng>(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    note: &AssetOrchardWalletNote,
    rng: &mut R,
) -> Result<AssetOrchardBoundedBytes, AssetOrchardError> {
    let pool_domain =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    validate_asset_orchard_wallet_note_for_pool(note, pool_domain)?;

    let g_d = secret_curve_value(note.note.g_d.to_affine()?);
    let pk_d = secret_curve_value(note.note.pk_d.to_affine()?);
    let esk = secret_curve_value(loop {
        let candidate = pallas::Scalar::random(&mut *rng);
        if !bool::from(candidate.is_zero()) {
            break candidate;
        }
    });
    let epk = (pallas::Point::from(g_d.0) * esk.0).to_affine();
    if bool::from(epk.is_identity()) {
        return Err(AssetOrchardError::new(
            "invalid_asset_orchard_note_ephemeral_key",
            "asset-orchard note encryption derived an identity ephemeral key",
        ));
    }
    let shared = secret_curve_value((pallas::Point::from(pk_d.0) * esk.0).to_affine());
    if bool::from(shared.0.is_identity()) {
        return Err(AssetOrchardError::new(
            "invalid_asset_orchard_note_shared_secret",
            "asset-orchard note encryption derived an identity shared secret",
        ));
    }

    let epk_bytes = epk.to_bytes();
    let pk_d_bytes = Zeroizing::new(pk_d.0.to_bytes());
    let shared_bytes = Zeroizing::new(shared.0.to_bytes());
    let key = Zeroizing::new(note_encryption_key(
        chain_id,
        genesis_hash,
        protocol_version,
        note.output_commitment.as_hex(),
        &epk_bytes,
        &pk_d_bytes,
        &shared_bytes,
    ));
    let aad = note_encryption_aad(
        chain_id,
        genesis_hash,
        protocol_version,
        note.output_commitment.as_hex(),
        &epk_bytes,
    );
    let plaintext = Zeroizing::new(serde_json::to_vec(note).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_note_plaintext_serialization_failed",
            error.to_string(),
        )
    })?);
    let mut nonce = [0u8; NONCE_BYTES];
    rng.fill_bytes(&mut nonce);
    let cipher = ChaCha20Poly1305::new((&*key).into());
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| {
            AssetOrchardError::new(
                "asset_orchard_note_encryption_failed",
                "asset-orchard note encryption failed",
            )
        })?;
    let mut encoded = Vec::with_capacity(HEADER_BYTES + ciphertext.len());
    encoded.extend_from_slice(ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC);
    encoded.extend_from_slice(&epk_bytes);
    encoded.extend_from_slice(&nonce);
    encoded.extend_from_slice(&ciphertext);
    AssetOrchardBoundedBytes::from_bytes(&encoded, ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES)
}

pub fn decrypt_asset_orchard_wallet_note(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    note_seed_hex: &str,
    expected_output_commitment: &str,
    encrypted_output: &[u8],
) -> Result<Option<AssetOrchardWalletNote>, AssetOrchardError> {
    if encrypted_output.len() < HEADER_BYTES + TAG_BYTES
        || &encrypted_output[..ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC.len()]
            != ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC
    {
        return Err(AssetOrchardError::new(
            "invalid_asset_orchard_note_ciphertext",
            "asset-orchard encrypted output has an invalid envelope",
        ));
    }
    let epk_start = ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC.len();
    let nonce_start = epk_start + EPK_BYTES;
    let ciphertext_start = nonce_start + NONCE_BYTES;
    let epk_bytes: [u8; EPK_BYTES] = encrypted_output[epk_start..nonce_start]
        .try_into()
        .expect("checked AssetOrchard ciphertext header");
    let epk = Option::<pallas::Affine>::from(pallas::Affine::from_bytes(&epk_bytes)).ok_or_else(
        || {
            AssetOrchardError::new(
                "invalid_asset_orchard_note_ephemeral_key",
                "asset-orchard encrypted output has a non-canonical ephemeral key",
            )
        },
    )?;
    if bool::from(epk.is_identity()) {
        return Err(AssetOrchardError::new(
            "invalid_asset_orchard_note_ephemeral_key",
            "asset-orchard encrypted output has an identity ephemeral key",
        ));
    }

    let ivk = asset_orchard_incoming_viewing_key_from_seed(note_seed_hex)?;
    let shared = secret_curve_value((pallas::Point::from(epk) * *ivk).to_affine());
    let pool_domain =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    let template = build_asset_orchard_wallet_note_with_rho(
        chain_id,
        genesis_hash,
        protocol_version,
        "scan-placeholder",
        1,
        note_seed_hex,
        pallas::Base::one(),
    )?;
    let pk_d_bytes = Zeroizing::new(template.note.pk_d.to_affine()?.to_bytes());
    let shared_bytes = Zeroizing::new(shared.0.to_bytes());
    let key = Zeroizing::new(note_encryption_key(
        chain_id,
        genesis_hash,
        protocol_version,
        expected_output_commitment,
        &epk_bytes,
        &pk_d_bytes,
        &shared_bytes,
    ));
    let aad = note_encryption_aad(
        chain_id,
        genesis_hash,
        protocol_version,
        expected_output_commitment,
        &epk_bytes,
    );
    let cipher = ChaCha20Poly1305::new((&*key).into());
    let Some(plaintext) = cipher
        .decrypt(
            Nonce::from_slice(&encrypted_output[nonce_start..ciphertext_start]),
            Payload {
                msg: &encrypted_output[ciphertext_start..],
                aad: &aad,
            },
        )
        .ok()
    else {
        return Ok(None);
    };
    let plaintext = Zeroizing::new(plaintext);
    // The typed copies (`note`, `template`, `expected`) wipe their opening and keys on drop.
    let note: AssetOrchardWalletNote = serde_json::from_slice(&plaintext).map_err(|error| {
        AssetOrchardError::new(
            "invalid_asset_orchard_note_plaintext",
            format!("decrypted asset-orchard note is invalid: {error}"),
        )
    })?;
    validate_asset_orchard_wallet_note_for_pool(&note, pool_domain)?;
    if note.output_commitment.as_hex() != expected_output_commitment {
        return Err(AssetOrchardError::new(
            "asset_orchard_note_ciphertext_commitment_mismatch",
            "decrypted asset-orchard note does not match the chain output commitment",
        ));
    }
    let rho = secret_curve_value(note.note.rho.to_field()?);
    let expected = build_asset_orchard_wallet_note_with_rho(
        chain_id,
        genesis_hash,
        protocol_version,
        &note.asset_id,
        note.value,
        note_seed_hex,
        rho.0,
    )?;
    if note != expected {
        return Err(AssetOrchardError::new(
            "asset_orchard_note_ciphertext_recipient_mismatch",
            "decrypted asset-orchard note is not controlled by the scanning key",
        ));
    }
    Ok(Some(note))
}

pub const ASSET_ORCHARD_TRANSFER_NOTE_CIPHERTEXT_MAGIC: &[u8; 8] = b"PFAOENC2";
const TRANSFER_NOTE_PLAINTEXT_SCHEMA: &str = "postfiat-asset-orchard-transfer-note-v1";
/// Every transfer output plaintext is padded to this size, so a ciphertext's length says
/// nothing about the asset or the amount inside.
const TRANSFER_NOTE_PLAINTEXT_BYTES: usize = 512;
pub const ASSET_ORCHARD_TRANSFER_OUTPUT_CIPHERTEXT_BYTES: usize =
    HEADER_BYTES + TRANSFER_NOTE_PLAINTEXT_BYTES + TAG_BYTES;

#[derive(serde::Serialize, serde::Deserialize)]
struct TransferNotePlaintext {
    schema: String,
    asset_id: String,
    value: u64,
    rseed: String,
    rho: String,
}

/// Encrypt what the owner of `address` needs to find and later spend an output: the asset, the
/// amount and the note's randomness. The sender never learns the recipient's keys.
#[allow(clippy::too_many_arguments)]
pub fn encrypt_asset_orchard_transfer_output(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    address: &AssetOrchardShieldedAddress,
    output_commitment_hex: &str,
    asset_id: &str,
    value: u64,
    rseed: &[u8; ASSET_ORCHARD_RSEED_BYTES],
    rho: pallas::Base,
) -> Result<AssetOrchardBoundedBytes, AssetOrchardError> {
    let rng = &mut OsRng;
    let esk = loop {
        let candidate = pallas::Scalar::random(&mut *rng);
        if !bool::from(candidate.is_zero()) {
            break candidate;
        }
    };
    let epk = (pallas::Point::from(address.g_d) * esk).to_affine();
    let shared = (pallas::Point::from(address.pk_d) * esk).to_affine();
    if bool::from(epk.is_identity()) || bool::from(shared.is_identity()) {
        return Err(AssetOrchardError::new(
            "invalid_asset_orchard_note_shared_secret",
            "asset-orchard transfer output encryption derived an identity point",
        ));
    }
    let epk_bytes = epk.to_bytes();
    let key = Zeroizing::new(note_encryption_key(
        chain_id,
        genesis_hash,
        protocol_version,
        output_commitment_hex,
        &epk_bytes,
        &address.pk_d.to_bytes(),
        &shared.to_bytes(),
    ));
    let aad = note_encryption_aad(
        chain_id,
        genesis_hash,
        protocol_version,
        output_commitment_hex,
        &epk_bytes,
    );
    let body = Zeroizing::new(
        serde_json::to_vec(&TransferNotePlaintext {
            schema: TRANSFER_NOTE_PLAINTEXT_SCHEMA.to_string(),
            asset_id: asset_id.to_string(),
            value,
            rseed: postfiat_crypto_provider::bytes_to_hex(rseed),
            rho: AssetOrchardFieldElement::from_field(rho).as_hex().to_string(),
        })
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_note_plaintext_serialization_failed",
                error.to_string(),
            )
        })?,
    );
    if body.len() + 2 > TRANSFER_NOTE_PLAINTEXT_BYTES {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_note_plaintext_too_large",
            "the transfer note does not fit the fixed plaintext size",
        ));
    }
    let mut plaintext = Zeroizing::new(vec![0u8; TRANSFER_NOTE_PLAINTEXT_BYTES]);
    plaintext[..2].copy_from_slice(&(body.len() as u16).to_le_bytes());
    plaintext[2..2 + body.len()].copy_from_slice(&body);
    let mut nonce = [0u8; NONCE_BYTES];
    rng.fill_bytes(&mut nonce);
    let cipher = ChaCha20Poly1305::new((&*key).into());
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| {
            AssetOrchardError::new(
                "asset_orchard_note_encryption_failed",
                "asset-orchard transfer output encryption failed",
            )
        })?;
    let mut encoded = Vec::with_capacity(ASSET_ORCHARD_TRANSFER_OUTPUT_CIPHERTEXT_BYTES);
    encoded.extend_from_slice(ASSET_ORCHARD_TRANSFER_NOTE_CIPHERTEXT_MAGIC);
    encoded.extend_from_slice(&epk_bytes);
    encoded.extend_from_slice(&nonce);
    encoded.extend_from_slice(&ciphertext);
    AssetOrchardBoundedBytes::from_bytes(&encoded, ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES)
}

/// Try one pool output with a wallet's viewing key. It needs only what the pool keeps for every
/// output: its commitment and its ciphertext. `Ok(None)` means the output is not this wallet's,
/// is a zero-value filler, is not a transfer output, or is malformed; a scan never fails on
/// someone else's bytes.
pub fn decrypt_asset_orchard_transfer_output(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    keys: &AssetOrchardShieldedKeys,
    output_commitment_hex: &str,
    encrypted_output: &[u8],
) -> Result<Option<AssetOrchardWalletNote>, AssetOrchardError> {
    let pool_domain =
        crate::asset_orchard::AssetOrchardTransferAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    let Ok(output_commitment) =
        AssetOrchardFieldElement::parse_hex(output_commitment_hex.to_string())
    else {
        return Ok(None);
    };
    if encrypted_output.len() != ASSET_ORCHARD_TRANSFER_OUTPUT_CIPHERTEXT_BYTES
        || &encrypted_output[..ASSET_ORCHARD_TRANSFER_NOTE_CIPHERTEXT_MAGIC.len()]
            != ASSET_ORCHARD_TRANSFER_NOTE_CIPHERTEXT_MAGIC
    {
        return Ok(None);
    }
    let epk_start = ASSET_ORCHARD_TRANSFER_NOTE_CIPHERTEXT_MAGIC.len();
    let nonce_start = epk_start + EPK_BYTES;
    let ciphertext_start = nonce_start + NONCE_BYTES;
    let epk_bytes: [u8; EPK_BYTES] = encrypted_output[epk_start..nonce_start]
        .try_into()
        .expect("checked transfer ciphertext header");
    let Some(epk) = Option::<pallas::Affine>::from(pallas::Affine::from_bytes(&epk_bytes)) else {
        return Ok(None);
    };
    if bool::from(epk.is_identity()) {
        return Ok(None);
    }
    let shared = (pallas::Point::from(epk) * keys.incoming_viewing_key()).to_affine();
    let commitment_hex = output_commitment.as_hex();
    let key = Zeroizing::new(note_encryption_key(
        chain_id,
        genesis_hash,
        protocol_version,
        commitment_hex,
        &epk_bytes,
        &keys.address.pk_d.to_bytes(),
        &shared.to_bytes(),
    ));
    let aad = note_encryption_aad(
        chain_id,
        genesis_hash,
        protocol_version,
        commitment_hex,
        &epk_bytes,
    );
    let cipher = ChaCha20Poly1305::new((&*key).into());
    let Some(plaintext) = cipher
        .decrypt(
            Nonce::from_slice(&encrypted_output[nonce_start..ciphertext_start]),
            Payload {
                msg: &encrypted_output[ciphertext_start..],
                aad: &aad,
            },
        )
        .ok()
    else {
        return Ok(None);
    };
    let plaintext = Zeroizing::new(plaintext);
    if plaintext.len() != TRANSFER_NOTE_PLAINTEXT_BYTES {
        return Ok(None);
    }
    let body_len = usize::from(u16::from_le_bytes([plaintext[0], plaintext[1]]));
    if body_len + 2 > plaintext.len() {
        return Ok(None);
    }
    let Ok(body) = serde_json::from_slice::<TransferNotePlaintext>(&plaintext[2..2 + body_len])
    else {
        return Ok(None);
    };
    if body.schema != TRANSFER_NOTE_PLAINTEXT_SCHEMA || body.value == 0 {
        return Ok(None);
    }
    let Ok(rseed_bytes) = postfiat_crypto_provider::hex_to_bytes(&body.rseed) else {
        return Ok(None);
    };
    let Ok(rseed) = <[u8; ASSET_ORCHARD_RSEED_BYTES]>::try_from(rseed_bytes.as_slice()) else {
        return Ok(None);
    };
    let Ok(asset_tag) = AssetTag::derive(&body.asset_id) else {
        return Ok(None);
    };
    let Ok(rho) = AssetOrchardFieldElement::parse_hex(body.rho.clone())
        .and_then(|rho| rho.to_field())
    else {
        return Ok(None);
    };
    let Ok(note) =
        asset_orchard_note_for_address(&keys.address, asset_tag, body.value, rho, &rseed)
    else {
        return Ok(None);
    };
    // A sender could encrypt a plaintext that does not open the published commitment. Such an
    // output is not spendable and must not be counted. The circuit fixed the commitment's rho,
    // so a plaintext with any other rho fails here too.
    if note.cmx(pool_domain)? != output_commitment.to_field()? {
        return Ok(None);
    }
    Ok(Some(keys.wallet_note(
        pool_domain,
        &body.asset_id,
        &note,
        &rseed,
    )?))
}

fn note_encryption_key(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    output_commitment: &str,
    epk: &[u8; 32],
    pk_d: &[u8; 32],
    shared: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    Digest::update(&mut hasher, KDF_DOMAIN);
    hash_context(
        &mut hasher,
        chain_id,
        genesis_hash,
        protocol_version,
        output_commitment,
        epk,
    );
    Digest::update(&mut hasher, pk_d);
    Digest::update(&mut hasher, shared);
    hasher.finalize().into()
}

fn note_encryption_aad(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    output_commitment: &str,
    epk: &[u8; 32],
) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(AAD_DOMAIN);
    aad.extend_from_slice(&(chain_id.len() as u32).to_le_bytes());
    aad.extend_from_slice(chain_id.as_bytes());
    aad.extend_from_slice(&genesis_hash);
    aad.extend_from_slice(&protocol_version.to_le_bytes());
    aad.extend_from_slice(&(output_commitment.len() as u32).to_le_bytes());
    aad.extend_from_slice(output_commitment.as_bytes());
    aad.extend_from_slice(epk);
    aad
}

fn hash_context(
    hasher: &mut Sha3_256,
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    output_commitment: &str,
    epk: &[u8; 32],
) {
    Digest::update(hasher, (chain_id.len() as u32).to_le_bytes());
    Digest::update(hasher, chain_id.as_bytes());
    Digest::update(hasher, genesis_hash);
    Digest::update(hasher, protocol_version.to_le_bytes());
    Digest::update(hasher, (output_commitment.len() as u32).to_le_bytes());
    Digest::update(hasher, output_commitment.as_bytes());
    Digest::update(hasher, epk);
}

#[cfg(test)]
mod tests {
    use postfiat_crypto_provider::bytes_to_hex;
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn recipient_recovers_note_from_chain_ciphertext_without_note_file() {
        let chain_id = "postfiat-note-encryption-test";
        let genesis_hash = [7u8; 32];
        let protocol_version = 1;
        let seed = bytes_to_hex(&[11u8; 32]);
        let note = crate::build_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            protocol_version,
            "asset-secret-label",
            42,
            &seed,
        )
        .expect("build recipient note");
        let mut rng = StdRng::from_seed([13u8; 32]);
        let encrypted = encrypt_asset_orchard_wallet_note_with_rng(
            chain_id,
            genesis_hash,
            protocol_version,
            &note,
            &mut rng,
        )
        .expect("encrypt recipient note")
        .to_bytes()
        .expect("ciphertext bytes");

        assert!(encrypted.starts_with(ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC));
        assert!(!encrypted
            .windows(b"asset-secret-label".len())
            .any(|window| window == b"asset-secret-label"));
        assert!(!encrypted
            .windows(note.output_commitment.as_hex().len())
            .any(|window| window == note.output_commitment.as_hex().as_bytes()));

        let recovered = decrypt_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            protocol_version,
            &seed,
            note.output_commitment.as_hex(),
            &encrypted,
        )
        .expect("scan ciphertext")
        .expect("recipient match");
        assert_eq!(recovered, note);

        let wrong_seed = bytes_to_hex(&[12u8; 32]);
        assert!(decrypt_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            protocol_version,
            &wrong_seed,
            note.output_commitment.as_hex(),
            &encrypted,
        )
        .expect("wrong recipient scan")
        .is_none());

        let mut tampered = encrypted;
        *tampered.last_mut().expect("ciphertext byte") ^= 1;
        assert!(decrypt_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            protocol_version,
            &seed,
            note.output_commitment.as_hex(),
            &tampered,
        )
        .expect("tampered scan")
        .is_none());
    }

    #[test]
    fn note_ciphertext_bytes_are_pinned() {
        let chain_id = "postfiat-note-encryption-test";
        let genesis_hash = [7u8; 32];
        let seed = bytes_to_hex(&[11u8; 32]);
        let note = crate::build_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            1,
            "asset-secret-label",
            42,
            &seed,
        )
        .expect("build recipient note");
        let encrypted = encrypt_asset_orchard_wallet_note_with_rng(
            chain_id,
            genesis_hash,
            1,
            &note,
            &mut StdRng::from_seed([13u8; 32]),
        )
        .expect("encrypt recipient note")
        .to_bytes()
        .expect("ciphertext bytes");
        assert_eq!(
            bytes_to_hex(&Sha3_256::digest(&encrypted)),
            "1df41ba0f38a66decb84814ec9c29f3088976b6a2f4b258693bf2c2df4d5c4d7"
        );
        let recovered = decrypt_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            1,
            &seed,
            note.output_commitment.as_hex(),
            &encrypted,
        )
        .expect("scan ciphertext")
        .expect("recipient match");
        assert!(recovered == note);
    }
}
