//! Unified v2 BIP-39 KDF (HKDF-SHA-384 domain-separated derivation) matching
//! `tasknode-apple#40` (`vectors/pq-unified-bip39-kdf.json`) and
//! `postfiat-privacy-binius-wallet::keys::derive`, while keeping existing v1
//! Nostr `npub` derivation intact.

use k256::schnorr::SigningKey as SchnorrSigningKey;
use postfiat_crypto_provider::{bytes_to_hex, hash_bytes, hex_to_bytes};
use postfiat_rpc_sdk::{wallet_backup_from_master_seed, wallet_identity_from_backup};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha384, Sha512};
use sha3::{
    digest::{ExtendableOutput as _, Update as _, XofReader as _},
    Shake256,
};
use zeroize::{Zeroize as _, Zeroizing};

pub const UNIFIED_KDF_VERSION: &str = "postfiat-unified-bip39-kdf-v1";
pub const UNIFIED_KDF_HKDF_SALT: &[u8] = b"postfiat/unified-bip39-kdf/v1/hkdf-salt";
pub const ROLE_PQ_BIP39_ROOT_SEED: &[u8] = b"postfiat/pq-wallet/v1/bip39-root-seed";
pub const ROLE_PQ_IMPORTED_MASTER_SEED: &[u8] = b"postfiat/pq-wallet/v1/imported-master-seed";
pub const ROLE_MESSAGE_KEY: &[u8] = b"postfiat-l1-v2/message-key/v1";
pub const ROLE_NOSTR_V2_SEED: &[u8] = b"postfiat.wallet.nostr.seed.v1";

// Exact domain constants matching `postfiat-privacy-binius-wallet/src/keys.rs` & `tasknode-apple`.
pub const PQ_WALLET_HKDF_SALT: &[u8] = b"postfiat/pq-wallet/v1/hkdf-salt";
pub const PQ_WALLET_ROLE_SPEND: &[u8] = b"postfiat/pq-wallet/v1/spend";
pub const PQ_WALLET_ROLE_KEM: &[u8] = b"postfiat/pq-wallet/v1/kem";
pub const PFSH03_SPEND_DSA_SEED_DOMAIN: &[u8] = b"PFT-SH-SPEND-DSA-SEED-V3";
pub const PFSH03_SPEND_RPK_DOMAIN: &[u8] = b"PFT-SH-SPEND-RPK-V3";

// Exact transparent ML-DSA-65 master-seed derivation constants matching `tasknode-apple` `pftl_v2.rs`.
pub const ML_DSA_BIP39_MASTER_SEED_DOMAIN: &str = "postfiat.wallet.bip39.master_seed.v1";
pub const ML_DSA_BIP39_DEFAULT_CHAIN_ID: &str = "postfiat-wan-devnet-2";
pub const ML_DSA_BIP39_DEFAULT_PROFILE: &str = "postfiat-controlled-testnet-v1";
pub const ML_DSA_BIP39_DEFAULT_PATH: &str = "m/44'/1'/0'/0/0";

const SHA384_BLOCK_LEN: usize = 128;
const SHA384_OUTPUT_LEN: usize = 48;
const SHA512_BLOCK_LEN: usize = 128;
const SHA512_OUTPUT_LEN: usize = 64;
const BIP39_PBKDF2_ROUNDS: u32 = 2048;

const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// RFC 2104 HMAC-SHA-384 with zeroized key pad buffers.
pub(crate) fn hmac_sha384(key: &[u8], parts: &[&[u8]]) -> Zeroizing<[u8; SHA384_OUTPUT_LEN]> {
    let mut key_block = Zeroizing::new([0u8; SHA384_BLOCK_LEN]);
    if key.len() > SHA384_BLOCK_LEN {
        let hashed = Zeroizing::new(<[u8; SHA384_OUTPUT_LEN]>::from(Sha384::digest(key)));
        key_block[..SHA384_OUTPUT_LEN].copy_from_slice(hashed.as_slice());
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = Zeroizing::new([0x36u8; SHA384_BLOCK_LEN]);
    let mut opad = Zeroizing::new([0x5cu8; SHA384_BLOCK_LEN]);
    for i in 0..SHA384_BLOCK_LEN {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }

    let mut inner = Sha384::new();
    inner.update(ipad.as_slice());
    for part in parts {
        inner.update(part);
    }
    let inner_digest = Zeroizing::new(<[u8; SHA384_OUTPUT_LEN]>::from(inner.finalize()));

    let mut outer = Sha384::new();
    outer.update(opad.as_slice());
    outer.update(inner_digest.as_slice());
    Zeroizing::new(<[u8; SHA384_OUTPUT_LEN]>::from(outer.finalize()))
}

/// RFC 5869 HKDF-Extract using HMAC-SHA-384.
pub(crate) fn hkdf_sha384_extract(salt: &[u8], ikm: &[u8]) -> Zeroizing<[u8; SHA384_OUTPUT_LEN]> {
    hmac_sha384(salt, &[ikm])
}

/// RFC 5869 HKDF-Expand using HMAC-SHA-384.
pub(crate) fn hkdf_sha384_expand(
    prk: &[u8; SHA384_OUTPUT_LEN],
    info: &[u8],
    out_len: usize,
) -> Result<Zeroizing<Vec<u8>>, String> {
    if out_len == 0 || out_len > 255 * SHA384_OUTPUT_LEN {
        return Err("invalid HKDF-SHA-384 output length".to_string());
    }
    let blocks = out_len.div_ceil(SHA384_OUTPUT_LEN);
    let mut okm = Zeroizing::new(Vec::with_capacity(blocks * SHA384_OUTPUT_LEN));
    let mut prev = Zeroizing::new(Vec::<u8>::new());
    for counter in 1..=blocks {
        let c = [counter as u8];
        let block = if prev.is_empty() {
            hmac_sha384(prk, &[info, &c])
        } else {
            hmac_sha384(prk, &[prev.as_slice(), info, &c])
        };
        okm.extend_from_slice(block.as_slice());
        prev.clear();
        prev.extend_from_slice(block.as_slice());
    }
    okm.truncate(out_len);
    Ok(okm)
}

/// RFC 2104 HMAC-SHA-512 with zeroized key pad buffers (used for BIP-39 PBKDF2).
fn hmac_sha512(key: &[u8], parts: &[&[u8]]) -> Zeroizing<[u8; SHA512_OUTPUT_LEN]> {
    let mut key_block = Zeroizing::new([0u8; SHA512_BLOCK_LEN]);
    if key.len() > SHA512_BLOCK_LEN {
        let hashed = Zeroizing::new(<[u8; SHA512_OUTPUT_LEN]>::from(Sha512::digest(key)));
        key_block[..SHA512_OUTPUT_LEN].copy_from_slice(hashed.as_slice());
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = Zeroizing::new([0x36u8; SHA512_BLOCK_LEN]);
    let mut opad = Zeroizing::new([0x5cu8; SHA512_BLOCK_LEN]);
    for i in 0..SHA512_BLOCK_LEN {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }

    let mut inner = Sha512::new();
    inner.update(ipad.as_slice());
    for part in parts {
        inner.update(part);
    }
    let inner_digest = Zeroizing::new(<[u8; SHA512_OUTPUT_LEN]>::from(inner.finalize()));

    let mut outer = Sha512::new();
    outer.update(opad.as_slice());
    outer.update(inner_digest.as_slice());
    Zeroizing::new(<[u8; SHA512_OUTPUT_LEN]>::from(outer.finalize()))
}

/// Compute the 64-byte BIP-39 seed via PBKDF2-HMAC-SHA512 (2048 iterations).
///
/// Callers MUST supply NFKD-normalized UTF-8 strings (e.g., `str.normalize('NFKD')` in JS).
pub fn bip39_seed64_from_nfkd(
    mnemonic_nfkd: &str,
    passphrase_nfkd: &str,
) -> Result<Zeroizing<[u8; 64]>, String> {
    let words: Vec<&str> = mnemonic_nfkd.split_whitespace().collect();
    if !matches!(words.len(), 12 | 15 | 18 | 21 | 24) {
        return Err("BIP-39 mnemonic must contain 12, 15, 18, 21, or 24 words".to_string());
    }
    if passphrase_nfkd.len() > 4096 {
        return Err("BIP-39 passphrase exceeds maximum length".to_string());
    }
    let password = Zeroizing::new(words.join(" ").to_lowercase());
    let mut salt = Zeroizing::new(Vec::with_capacity(8 + passphrase_nfkd.len()));
    salt.extend_from_slice(b"mnemonic");
    salt.extend_from_slice(passphrase_nfkd.as_bytes());

    // Single PBKDF2 block (64 bytes == SHA-512 output length) with block index 1.
    let mut u = hmac_sha512(password.as_bytes(), &[salt.as_slice(), &1u32.to_be_bytes()]);
    let mut out = Zeroizing::new(*u);
    for _ in 1..BIP39_PBKDF2_ROUNDS {
        u = hmac_sha512(password.as_bytes(), &[u.as_slice()]);
        for (dst, src) in out.iter_mut().zip(u.iter()) {
            *dst ^= *src;
        }
    }
    Ok(out)
}

fn push_framed(dst: &mut Vec<u8>, bytes: &[u8]) {
    dst.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    dst.extend_from_slice(bytes);
}

fn decode_genesis_48(chain_id: &str, genesis_hash_hex: &str) -> Result<[u8; 48], String> {
    if chain_id.is_empty() || chain_id.len() > 255 {
        return Err("invalid chain_id".to_string());
    }
    let raw = hex_to_bytes(genesis_hash_hex).map_err(|e| format!("invalid genesis_hash: {e}"))?;
    raw.try_into()
        .map_err(|_| "genesis_hash must decode to 48 bytes".to_string())
}

fn network_frame(chain_id: &str, genesis_hash_hex: &str) -> Result<Vec<u8>, String> {
    let genesis = decode_genesis_48(chain_id, genesis_hash_hex)?;
    postfiat_types::pq_address::network_frame(chain_id, &genesis).map_err(str::to_string)
}

pub fn decode_bip39_seed64_hex(bip39_seed64_hex: &str) -> Result<Zeroizing<[u8; 64]>, String> {
    let bytes = Zeroizing::new(
        hex_to_bytes(bip39_seed64_hex).map_err(|e| format!("invalid bip39_seed64_hex: {e}"))?,
    );
    let arr: [u8; 64] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| "bip39_seed64_hex must decode to 64 bytes".to_string())?;
    Ok(Zeroizing::new(arr))
}

/// Derive the 32-byte transparent ML-DSA-65 master seed from a 64-byte BIP-39 seed.
///
/// Matches `tasknode-apple`'s `pftl_v2::master_seed` byte-for-byte.
pub fn derive_ml_dsa_master_seed_from_bip39(
    bip39_seed64: &[u8],
) -> Result<Zeroizing<[u8; 32]>, String> {
    if bip39_seed64.len() != 64 {
        return Err("bip39_seed64 must be 64 bytes".to_string());
    }
    let seed_hex = Zeroizing::new(bytes_to_hex(bip39_seed64));
    let payload = Zeroizing::new(
        serde_json::to_vec(&(
            seed_hex.as_str(),
            ML_DSA_BIP39_DEFAULT_CHAIN_ID,
            ML_DSA_BIP39_DEFAULT_PROFILE,
            ML_DSA_BIP39_DEFAULT_PATH,
            0u32,
            0u32,
        ))
        .map_err(|e| format!("serialize master seed preimage: {e}"))?,
    );
    let digest = Zeroizing::new(hash_bytes(ML_DSA_BIP39_MASTER_SEED_DOMAIN, &payload));
    let mut out = Zeroizing::new([0u8; 32]);
    out.copy_from_slice(&digest[..32]);
    Ok(out)
}

/// Derive the unified 48-byte PFSH03 PQ root seed from a 64-byte BIP-39 seed.
pub fn derive_pq_root_seed_from_bip39(
    bip39_seed64: &[u8],
    chain_id: &str,
    genesis_hash_hex: &str,
    account_index: u32,
) -> Result<Zeroizing<[u8; 48]>, String> {
    if bip39_seed64.len() != 64 {
        return Err("bip39_seed64 must be 64 bytes".to_string());
    }
    let net = network_frame(chain_id, genesis_hash_hex)?;
    let prk = hkdf_sha384_extract(UNIFIED_KDF_HKDF_SALT, bip39_seed64);
    let mut info = Vec::with_capacity(64 + net.len() + 8);
    push_framed(&mut info, ROLE_PQ_BIP39_ROOT_SEED);
    info.extend_from_slice(&net);
    push_framed(&mut info, &account_index.to_be_bytes());
    let okm = hkdf_sha384_expand(&prk, &info, 48)?;
    let mut out = Zeroizing::new([0u8; 48]);
    out.copy_from_slice(okm.as_slice());
    Ok(out)
}

/// Derive a domain-separated 48-byte PFSH03 PQ root seed for an imported 32-byte L1 master seed.
pub fn derive_pq_root_seed_from_imported_master(
    master_seed32: &[u8],
    chain_id: &str,
    genesis_hash_hex: &str,
    account_index: u32,
) -> Result<Zeroizing<[u8; 48]>, String> {
    if master_seed32.len() != 32 {
        return Err("master_seed32 must be 32 bytes".to_string());
    }
    let net = network_frame(chain_id, genesis_hash_hex)?;
    let prk = hkdf_sha384_extract(UNIFIED_KDF_HKDF_SALT, master_seed32);
    let mut info = Vec::with_capacity(64 + net.len() + 8);
    push_framed(&mut info, ROLE_PQ_IMPORTED_MASTER_SEED);
    info.extend_from_slice(&net);
    push_framed(&mut info, &account_index.to_be_bytes());
    let okm = hkdf_sha384_expand(&prk, &info, 48)?;
    let mut out = Zeroizing::new([0u8; 48]);
    out.copy_from_slice(okm.as_slice());
    Ok(out)
}

/// Derived PFSH03 secret material (zeroized on drop; redacted in `Debug`).
pub struct UnifiedPfsh03Material {
    pub pq_seed: Zeroizing<[u8; 48]>,
    pub spend_secret: Zeroizing<[u8; 48]>,
    pub spend_dsa_seed: Zeroizing<[u8; 32]>,
    pub spend_r_pk: Zeroizing<[u8; 48]>,
    pub scan_kem_seed: Zeroizing<[u8; 64]>,
}

impl std::fmt::Debug for UnifiedPfsh03Material {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedPfsh03Material")
            .field("pq_seed", &"<redacted>")
            .field("spend_secret", &"<redacted>")
            .field("spend_dsa_seed", &"<redacted>")
            .field("spend_r_pk", &"<redacted>")
            .field("scan_kem_seed", &"<redacted>")
            .finish()
    }
}

/// Derive PFSH03 spend secret, spend DSA seed, spend `r_pk`, and ML-KEM-1024 scan seed.
///
/// Byte-for-byte identical to `postfiat-privacy-binius-wallet::keys::derive` and `tasknode-apple`.
pub fn derive_pfsh03_material(
    pq_seed: &[u8; 48],
    chain_id: &str,
    genesis_hash_hex: &str,
    account_index: u32,
    address_index: u32,
) -> Result<UnifiedPfsh03Material, String> {
    let net = network_frame(chain_id, genesis_hash_hex)?;
    let prk = hkdf_sha384_extract(PQ_WALLET_HKDF_SALT, pq_seed);

    let expand_role = |label: &[u8], out_len: usize| -> Result<Zeroizing<Vec<u8>>, String> {
        let mut info = Vec::with_capacity(4 + label.len() + net.len() + 16);
        push_framed(&mut info, label);
        info.extend_from_slice(&net);
        push_framed(&mut info, &account_index.to_be_bytes());
        push_framed(&mut info, &address_index.to_be_bytes());
        hkdf_sha384_expand(&prk, &info, out_len)
    };

    let spend_vec = expand_role(PQ_WALLET_ROLE_SPEND, 48)?;
    let kem_vec = expand_role(PQ_WALLET_ROLE_KEM, 64)?;

    let mut spend_secret = Zeroizing::new([0u8; 48]);
    spend_secret.copy_from_slice(spend_vec.as_slice());

    let mut scan_kem_seed = Zeroizing::new([0u8; 64]);
    scan_kem_seed.copy_from_slice(kem_vec.as_slice());

    let mut dsa_xof = Shake256::default();
    dsa_xof.update(PFSH03_SPEND_DSA_SEED_DOMAIN);
    dsa_xof.update(spend_secret.as_slice());
    let mut spend_dsa_seed = Zeroizing::new([0u8; 32]);
    dsa_xof.finalize_xof().read(spend_dsa_seed.as_mut());

    let mut rpk_xof = Shake256::default();
    rpk_xof.update(PFSH03_SPEND_RPK_DOMAIN);
    rpk_xof.update(spend_secret.as_slice());
    let mut spend_r_pk = Zeroizing::new([0u8; 48]);
    rpk_xof.finalize_xof().read(spend_r_pk.as_mut());

    Ok(UnifiedPfsh03Material {
        pq_seed: Zeroizing::new(*pq_seed),
        spend_secret,
        spend_dsa_seed,
        spend_r_pk,
        scan_kem_seed,
    })
}

/// Derive a domain-separated sibling role key from the 64-byte BIP-39 root seed.
///
/// - Pillar 2 encrypted message key: `role = ROLE_MESSAGE_KEY`, `chain_id = Some(chain_id)`.
/// - Pillar 6 Nostr v2 seed: `role = ROLE_NOSTR_V2_SEED`, `chain_id = None` (survives chain upgrades).
pub fn derive_role_key_bytes(
    bip39_seed64: &[u8],
    role: &[u8],
    chain_id: Option<&str>,
    account_index: u32,
    key_index: u32,
    out_len: usize,
) -> Result<Zeroizing<Vec<u8>>, String> {
    if bip39_seed64.len() != 64 || role.is_empty() {
        return Err("invalid role key derivation input".to_string());
    }
    let prk = hkdf_sha384_extract(UNIFIED_KDF_HKDF_SALT, bip39_seed64);
    let mut info = Vec::with_capacity(64 + role.len());
    push_framed(&mut info, role);
    if let Some(chain) = chain_id {
        if chain.is_empty() || chain.len() > 255 {
            return Err("invalid chain_id".to_string());
        }
        push_framed(&mut info, b"chain-bound");
        push_framed(&mut info, chain.as_bytes());
    } else {
        push_framed(&mut info, b"chain-agnostic");
    }
    push_framed(&mut info, &account_index.to_be_bytes());
    push_framed(&mut info, &key_index.to_be_bytes());
    hkdf_sha384_expand(&prk, &info, out_len)
}

pub fn derive_message_key_seed(
    bip39_seed64: &[u8],
    chain_id: &str,
    account_index: u32,
    key_index: u32,
) -> Result<Zeroizing<[u8; 32]>, String> {
    let bytes = derive_role_key_bytes(
        bip39_seed64,
        ROLE_MESSAGE_KEY,
        Some(chain_id),
        account_index,
        key_index,
        32,
    )?;
    let mut out = Zeroizing::new([0u8; 32]);
    out.copy_from_slice(bytes.as_slice());
    Ok(out)
}

pub fn derive_nostr_v2_seed(
    bip39_seed64: &[u8],
    account_index: u32,
    key_index: u32,
) -> Result<Zeroizing<[u8; 32]>, String> {
    let bytes = derive_role_key_bytes(
        bip39_seed64,
        ROLE_NOSTR_V2_SEED,
        None,
        account_index,
        key_index,
        32,
    )?;
    let mut out = Zeroizing::new([0u8; 32]);
    out.copy_from_slice(bytes.as_slice());
    Ok(out)
}

/// Nostr secp256k1 key material (v1 legacy or v2 unified KDF). Secret key is redacted in `Debug`.
pub struct NostrKeyMaterial {
    pub secret_key: Zeroizing<[u8; 32]>,
    pub pubkey_xonly_hex: String,
    pub npub: String,
    pub derivation_version: u8,
    pub account_index: u32,
    pub key_index: u32,
}

impl std::fmt::Debug for NostrKeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NostrKeyMaterial")
            .field("secret_key", &"<redacted>")
            .field("pubkey_xonly_hex", &self.pubkey_xonly_hex)
            .field("npub", &self.npub)
            .field("derivation_version", &self.derivation_version)
            .field("account_index", &self.account_index)
            .field("key_index", &self.key_index)
            .finish()
    }
}

/// Public-only Nostr identity projection (safe to return across JS/WASM boundary).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NostrPublicIdentity {
    pub derivation_version: u8,
    pub account_index: u32,
    pub key_index: u32,
    pub nostr_pubkey_hex: String,
    pub npub: String,
}

/// Derive a Pillar 6 v2 Nostr BIP-340 keypair directly from the 64-byte BIP-39 seed.
pub fn derive_nostr_v2_keypair(
    bip39_seed64: &[u8],
    account_index: u32,
    key_index: u32,
) -> Result<NostrKeyMaterial, String> {
    let seed = derive_nostr_v2_seed(bip39_seed64, account_index, key_index)?;
    let signing_key = SchnorrSigningKey::from_bytes(seed.as_slice())
        .map_err(|e| format!("invalid derived secp256k1 scalar: {e}"))?;
    let xonly = signing_key.verifying_key().to_bytes();
    let pubkey_xonly_hex = bytes_to_hex(xonly.as_slice());
    let npub = encode_npub(&pubkey_xonly_hex)?;
    Ok(NostrKeyMaterial {
        secret_key: seed,
        pubkey_xonly_hex,
        npub,
        derivation_version: 2,
        account_index,
        key_index,
    })
}

/// Build Nostr key material from an existing 32-byte secret key (preserving v1 `npub` derivation).
pub fn nostr_v1_keypair_from_secret_hex(
    secret_key_hex: &str,
    key_index: u32,
) -> Result<NostrKeyMaterial, String> {
    let bytes = Zeroizing::new(
        hex_to_bytes(secret_key_hex).map_err(|e| format!("invalid nostr secret hex: {e}"))?,
    );
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| "nostr secret key must be 32 bytes".to_string())?;
    let secret = Zeroizing::new(arr);
    let signing_key = SchnorrSigningKey::from_bytes(secret.as_slice())
        .map_err(|e| format!("invalid secp256k1 secret key: {e}"))?;
    let xonly = signing_key.verifying_key().to_bytes();
    let pubkey_xonly_hex = bytes_to_hex(xonly.as_slice());
    let npub = encode_npub(&pubkey_xonly_hex)?;
    Ok(NostrKeyMaterial {
        secret_key: secret,
        pubkey_xonly_hex,
        npub,
        derivation_version: 1,
        account_index: 0,
        key_index,
    })
}

/// Public-only unified KDF preview (zero secrets exposed across JS/WASM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnifiedKdfPublicPreview {
    pub kdf_version: String,
    pub chain_id: String,
    pub account_index: u32,
    pub address_index: u32,
    pub ml_dsa_address: String,
    pub ml_dsa_public_key_hex: String,
    pub nostr_v2_pubkey_hex: String,
    pub nostr_v2_npub: String,
}

/// Wallet derivation output returned when creating/restoring a local encrypted browser vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnifiedKdfWalletInit {
    pub kdf_version: String,
    pub chain_id: String,
    pub account_index: u32,
    pub address_index: u32,
    pub ml_dsa_address: String,
    pub ml_dsa_public_key_hex: String,
    pub nostr_v2_pubkey_hex: String,
    pub nostr_v2_npub: String,
    pub backup_json: String,
}

pub fn derive_unified_public_preview(
    bip39_seed64: &[u8],
    chain_id: &str,
    genesis_hash_hex: &str,
    account_index: u32,
    address_index: u32,
) -> Result<UnifiedKdfPublicPreview, String> {
    // Validate domain frame even for public preview so invalid genesis hashes fail closed.
    let _ = network_frame(chain_id, genesis_hash_hex)?;
    let master_seed = derive_ml_dsa_master_seed_from_bip39(bip39_seed64)?;
    let master_seed_hex = Zeroizing::new(bytes_to_hex(master_seed.as_slice()));
    let mut backup =
        wallet_backup_from_master_seed(chain_id, master_seed_hex.as_str(), account_index)
            .map_err(|e| e.to_string())?;
    let identity_res = wallet_identity_from_backup(&backup).map_err(|e| e.to_string());
    backup.master_seed_hex.zeroize();
    let identity = identity_res?;
    let nostr = derive_nostr_v2_keypair(bip39_seed64, account_index, address_index)?;
    Ok(UnifiedKdfPublicPreview {
        kdf_version: UNIFIED_KDF_VERSION.to_string(),
        chain_id: chain_id.to_string(),
        account_index,
        address_index,
        ml_dsa_address: identity.address,
        ml_dsa_public_key_hex: identity.public_key_hex,
        nostr_v2_pubkey_hex: nostr.pubkey_xonly_hex,
        nostr_v2_npub: nostr.npub,
    })
}

pub fn derive_unified_wallet_init(
    bip39_seed64: &[u8],
    chain_id: &str,
    genesis_hash_hex: &str,
    account_index: u32,
    address_index: u32,
) -> Result<UnifiedKdfWalletInit, String> {
    let _ = network_frame(chain_id, genesis_hash_hex)?;
    let master_seed = derive_ml_dsa_master_seed_from_bip39(bip39_seed64)?;
    let master_seed_hex = Zeroizing::new(bytes_to_hex(master_seed.as_slice()));
    let mut backup =
        wallet_backup_from_master_seed(chain_id, master_seed_hex.as_str(), account_index)
            .map_err(|e| e.to_string())?;
    let identity_res = wallet_identity_from_backup(&backup).map_err(|e| e.to_string());
    let backup_json_res = serde_json::to_string(&backup).map_err(|e| e.to_string());
    backup.master_seed_hex.zeroize();
    let identity = identity_res?;
    let backup_json = backup_json_res?;
    let nostr = derive_nostr_v2_keypair(bip39_seed64, account_index, address_index)?;
    Ok(UnifiedKdfWalletInit {
        kdf_version: UNIFIED_KDF_VERSION.to_string(),
        chain_id: chain_id.to_string(),
        account_index,
        address_index,
        ml_dsa_address: identity.address,
        ml_dsa_public_key_hex: identity.public_key_hex,
        nostr_v2_pubkey_hex: nostr.pubkey_xonly_hex,
        nostr_v2_npub: nostr.npub,
        backup_json,
    })
}

// ---------------------------------------------------------------------------
// BIP-173 Bech32 encoding/decoding for NIP-19 `npub`
// ---------------------------------------------------------------------------

fn bech32_polymod(values: &[u8]) -> u32 {
    const GEN: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
    let mut chk: u32 = 1;
    for &v in values {
        let top = chk >> 25;
        chk = ((chk & 0x01ff_ffff) << 5) ^ u32::from(v);
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 != 0 {
                chk ^= g;
            }
        }
    }
    chk
}

fn bech32_hrp_expand(hrp: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(hrp.len() * 2 + 1);
    for b in hrp.bytes() {
        out.push(b >> 5);
    }
    out.push(0);
    for b in hrp.bytes() {
        out.push(b & 31);
    }
    out
}

fn convert_bits(data: &[u8], from_bits: u32, to_bits: u32, pad: bool) -> Result<Vec<u8>, String> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let maxv: u32 = (1 << to_bits) - 1;
    let mut out = Vec::with_capacity((data.len() * from_bits as usize).div_ceil(to_bits as usize));
    for &value in data {
        let v = u32::from(value);
        if (v >> from_bits) != 0 {
            return Err("invalid bit group".to_string());
        }
        acc = (acc << from_bits) | v;
        bits += from_bits;
        while bits >= to_bits {
            bits -= to_bits;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to_bits - bits)) & maxv) as u8);
        }
    } else if bits >= from_bits || ((acc << (to_bits - bits)) & maxv) != 0 {
        return Err("invalid bech32 padding".to_string());
    }
    Ok(out)
}

/// Encode a 32-byte x-only Nostr public key (64 lowercase hex) as NIP-19 bech32 `npub1...`.
pub fn encode_npub(xonly_pubkey_hex: &str) -> Result<String, String> {
    if xonly_pubkey_hex.len() != 64
        || !xonly_pubkey_hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("nostr pubkey must be 64 lowercase hex chars".to_string());
    }
    let raw = hex_to_bytes(xonly_pubkey_hex).map_err(|e| e.to_string())?;
    let data = convert_bits(&raw, 8, 5, true)?;
    let mut values = bech32_hrp_expand("npub");
    values.extend_from_slice(&data);
    values.extend_from_slice(&[0u8; 6]);
    let polymod = bech32_polymod(&values) ^ 1;
    let mut encoded = String::with_capacity(5 + data.len() + 6);
    encoded.push_str("npub1");
    for v in data {
        encoded.push(BECH32_CHARSET[v as usize] as char);
    }
    for i in 0..6 {
        let idx = ((polymod >> (5 * (5 - i))) & 31) as usize;
        encoded.push(BECH32_CHARSET[idx] as char);
    }
    Ok(encoded)
}

/// Decode a NIP-19 bech32 `npub1...` into a 64-char lowercase hex x-only public key.
pub fn decode_npub(npub: &str) -> Result<String, String> {
    let lower = npub.to_ascii_lowercase();
    if npub != lower && npub != npub.to_ascii_uppercase() {
        return Err("bech32 string must not mix case".to_string());
    }
    let Some(data_part) = lower.strip_prefix("npub1") else {
        return Err("invalid npub prefix".to_string());
    };
    if data_part.len() < 6 {
        return Err("npub too short".to_string());
    }
    let mut data = Vec::with_capacity(data_part.len());
    for b in data_part.bytes() {
        let pos = BECH32_CHARSET
            .iter()
            .position(|&c| c == b)
            .ok_or_else(|| "invalid bech32 character".to_string())?;
        data.push(pos as u8);
    }
    let mut values = bech32_hrp_expand("npub");
    values.extend_from_slice(&data);
    if bech32_polymod(&values) != 1 {
        return Err("invalid npub checksum".to_string());
    }
    let payload = &data[..data.len() - 6];
    let raw = convert_bits(payload, 5, 8, false)?;
    if raw.len() != 32 {
        return Err("npub payload must decode to 32 bytes".to_string());
    }
    Ok(bytes_to_hex(&raw))
}
