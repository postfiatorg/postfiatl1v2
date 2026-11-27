//! Pillar 4 NFT signing helpers for `wallet_wasm`:
//! - Canonical deterministic protobuf3 encoding for `pf.nft.metadata.v1`, `pf.nft.claim.v1`,
//!   and Pillar 2 `postfiat.message.v1.Envelope` (Contract C1).
//! - ML-DSA-65 envelope signing & zero-re-serialization verification enforcing the one-hash rule
//!   (`metadata_hash == SHA3-384(raw_envelope_bytes)` and `account(sender_pubkey) == nft.issuer`).
//! - Deterministic bit-63 migrated serial calculation (`compute_nft_migrated_serial`).
//! - On-chain `nft_mint` / `nft_transfer` / `nft_burn` payload builders & ML-DSA-65 signers.

use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hex_to_bytes, ml_dsa_65_sign_with_context,
    ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM,
};
use postfiat_rpc_sdk::{
    derive_wallet_key_pair, wallet_identity_from_backup, wallet_sign_nft_transaction_from_fields,
    wallet_sign_nft_transaction_from_quote, RpcResponse, WalletBackupFile,
    WalletSignNftTransactionFields,
};
use postfiat_types::{
    nft_id, NftBurnOperation, NftMintOperation, NftTransferOperation, SignedNftTransaction,
    MAX_NFT_METADATA_URI_BYTES, NFT_COLLECTION_FLAG_BURN_LOCKED,
    NFT_COLLECTION_FLAG_TRANSFER_LOCKED, NFT_FLAG_TRANSFERABLE,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sha3::{Digest as _, Sha3_384};
use std::collections::BTreeMap;

pub const DOMAIN_ENVELOPE_SIGN: &[u8] = b"postfiat-l1-v2/message-envelope/v1";
pub const DOMAIN_NFT_MIGRATED_SERIAL: &[u8] = b"postfiat.nft.migrated_serial.v1";

pub const NFT_METADATA_SCHEMA_NAME: &str = "pf.nft.metadata.v1";
pub const NFT_METADATA_SCHEMA_ID: u32 = 400;
pub const NFT_CLAIM_SCHEMA_NAME: &str = "pf.nft.claim.v1";
pub const NFT_CLAIM_SCHEMA_ID: u32 = 401;

pub const CONTENT_KIND_NFT: u32 = 13;
pub const VISIBILITY_PUBLIC: u32 = 1;
pub const BODY_ENCODING_PROTOBUF: u32 = 1;
pub const CIPHER_SUITE_NONE: u32 = 1;
pub const SIGNATURE_ALGORITHM_ML_DSA_65: u32 = 1;
pub const MAX_SINGLE_ENVELOPE_BYTES: usize = 1_048_576;
pub const NFT_MIGRATED_SERIAL_BIT: u64 = 1_u64 << 63;

const BASE32_LOWER: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

// ---------------------------------------------------------------------------
// Canonical deterministic protobuf3 wire primitives (Contract C1 §2.1)
// ---------------------------------------------------------------------------

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    while value > 0x7f {
        out.push(((value & 0x7f) as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn decode_varint(buf: &[u8], offset: &mut usize) -> Result<u64, String> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        if *offset >= buf.len() {
            return Err("truncated protobuf varint".to_string());
        }
        let byte = buf[*offset];
        *offset += 1;
        if shift > 63 || (shift == 63 && byte > 1) {
            return Err("protobuf varint overflow".to_string());
        }
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
    }
}

fn pb_varint(field_num: u32, value: u64, out: &mut Vec<u8>) {
    if value == 0 {
        return;
    }
    encode_varint(u64::from(field_num) << 3, out);
    encode_varint(value, out);
}

fn pb_bool(field_num: u32, value: bool, out: &mut Vec<u8>) {
    if value {
        pb_varint(field_num, 1, out);
    }
}

fn pb_bytes(field_num: u32, value: &[u8], out: &mut Vec<u8>) {
    if value.is_empty() {
        return;
    }
    encode_varint((u64::from(field_num) << 3) | 2, out);
    encode_varint(value.len() as u64, out);
    out.extend_from_slice(value);
}

fn pb_string(field_num: u32, value: &str, out: &mut Vec<u8>) {
    pb_bytes(field_num, value.as_bytes(), out);
}

fn pb_map_ss(field_num: u32, map: &BTreeMap<String, String>, out: &mut Vec<u8>) {
    // BTreeMap iterates in lexicographic UTF-8 byte order (Contract C1 §2.1 rule 4).
    for (k, v) in map {
        let mut entry = Vec::with_capacity(k.len() + v.len() + 8);
        pb_string(1, k, &mut entry);
        pb_string(2, v, &mut entry);
        encode_varint((u64::from(field_num) << 3) | 2, out);
        encode_varint(entry.len() as u64, out);
        out.extend_from_slice(&entry);
    }
}

enum WireValue<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

fn decode_wire_fields(buf: &[u8]) -> Result<Vec<(u32, WireValue<'_>)>, String> {
    let mut fields = Vec::new();
    let mut offset = 0usize;
    while offset < buf.len() {
        let tag = decode_varint(buf, &mut offset)?;
        let field_num = u32::try_from(tag >> 3).map_err(|_| "field number overflow".to_string())?;
        let wire_type = tag & 0x07;
        if field_num == 0 {
            return Err("invalid protobuf field number 0".to_string());
        }
        match wire_type {
            0 => {
                let value = decode_varint(buf, &mut offset)?;
                fields.push((field_num, WireValue::Varint(value)));
            }
            2 => {
                let len = usize::try_from(decode_varint(buf, &mut offset)?)
                    .map_err(|_| "length overflow".to_string())?;
                if offset.checked_add(len).is_none_or(|end| end > buf.len()) {
                    return Err("truncated protobuf length-delimited field".to_string());
                }
                let slice = &buf[offset..offset + len];
                offset += len;
                fields.push((field_num, WireValue::Bytes(slice)));
            }
            1 => {
                if offset + 8 > buf.len() {
                    return Err("truncated 64-bit protobuf field".to_string());
                }
                offset += 8;
            }
            5 => {
                if offset + 4 > buf.len() {
                    return Err("truncated 32-bit protobuf field".to_string());
                }
                offset += 4;
            }
            _ => return Err(format!("unsupported protobuf wire type {wire_type}")),
        }
    }
    Ok(fields)
}

// ---------------------------------------------------------------------------
// IPFS CIDv1 raw + SHA3-384 + migrated serial primitives
// ---------------------------------------------------------------------------

fn base32_lower_nopad(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() * 8).div_ceil(5));
    let mut buffer: u32 = 0;
    let mut bits_left: u32 = 0;
    for &byte in input {
        buffer = (buffer << 8) | u32::from(byte);
        bits_left += 8;
        while bits_left >= 5 {
            bits_left -= 5;
            let idx = ((buffer >> bits_left) & 0x1f) as usize;
            out.push(BASE32_LOWER[idx] as char);
        }
    }
    if bits_left > 0 {
        let idx = ((buffer << (5 - bits_left)) & 0x1f) as usize;
        out.push(BASE32_LOWER[idx] as char);
    }
    out
}

/// Compute IPFS CIDv1 raw (`0x01 || 0x55 || 0x12 || 0x20 || SHA-256(block)`).
pub fn cidv1_raw_from_bytes(block: &[u8]) -> ([u8; 36], String) {
    let digest = Sha256::digest(block);
    let mut cid_bin = [0u8; 36];
    cid_bin[0] = 0x01;
    cid_bin[1] = 0x55;
    cid_bin[2] = 0x12;
    cid_bin[3] = 0x20;
    cid_bin[4..].copy_from_slice(digest.as_slice());
    let multibase = format!("b{}", base32_lower_nopad(&cid_bin));
    (cid_bin, multibase)
}

pub fn sha3_384_hex(data: &[u8]) -> String {
    let digest = Sha3_384::digest(data);
    bytes_to_hex(digest.as_slice())
}

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    bytes_to_hex(digest.as_slice())
}

/// Deterministic bit-63-partitioned migrated NFT serial (Contract C1 §6.2 / Train A §3.1).
pub fn compute_nft_migrated_serial(source_chain: &str, nftoken_id: &str) -> Result<u64, String> {
    if source_chain.is_empty() || nftoken_id.is_empty() {
        return Err("source_chain and nftoken_id must be non-empty".to_string());
    }
    let mut hasher = Sha3_384::new();
    hasher.update(DOMAIN_NFT_MIGRATED_SERIAL);
    hasher.update([0u8]);
    hasher.update(source_chain.as_bytes());
    hasher.update([0u8]);
    hasher.update(nftoken_id.to_ascii_uppercase().as_bytes());
    let digest = hasher.finalize();
    let head = u64::from_be_bytes(digest[..8].try_into().expect("8-byte slice"));
    Ok(NFT_MIGRATED_SERIAL_BIT | (head & 0x7fff_ffff_ffff_ffff))
}

/// Raw-byte ML-DSA-65 signing preimage for `postfiat.message.v1.Envelope` (Contract C1 §4.1).
pub fn envelope_signing_preimage(
    version: u32,
    header_bytes: &[u8],
    encryption_header_bytes: &[u8],
    body_bytes: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        DOMAIN_ENVELOPE_SIGN.len()
            + 1
            + 16
            + header_bytes.len()
            + encryption_header_bytes.len()
            + body_bytes.len(),
    );
    out.extend_from_slice(DOMAIN_ENVELOPE_SIGN);
    out.push(0u8);
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&(header_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(header_bytes);
    out.extend_from_slice(&(encryption_header_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(encryption_header_bytes);
    out.extend_from_slice(&(body_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(body_bytes);
    out
}

pub fn encode_public_message_header(
    chain_id: &str,
    schema_name: &str,
    schema_id: u32,
    kind: u32,
    sender_account: &str,
    created_at_unix_ms: u64,
) -> Vec<u8> {
    let mut out = Vec::new();
    pb_varint(1, 1, &mut out); // protocol_version = 1
    pb_string(2, chain_id, &mut out);
    pb_string(3, schema_name, &mut out);
    pb_varint(4, u64::from(schema_id), &mut out);
    pb_varint(5, u64::from(kind), &mut out);
    pb_varint(6, u64::from(VISIBILITY_PUBLIC), &mut out);
    pb_varint(7, u64::from(BODY_ENCODING_PROTOBUF), &mut out);
    pb_varint(8, u64::from(CIPHER_SUITE_NONE), &mut out);
    pb_string(9, sender_account, &mut out);
    pb_varint(10, created_at_unix_ms, &mut out);
    out
}

pub fn encode_envelope(
    version: u32,
    header_bytes: &[u8],
    encryption_header_bytes: &[u8],
    body_bytes: &[u8],
    signature_algorithm: u32,
    sender_pubkey: &[u8],
    signature: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        header_bytes.len()
            + encryption_header_bytes.len()
            + body_bytes.len()
            + sender_pubkey.len()
            + signature.len()
            + 32,
    );
    pb_varint(1, u64::from(version), &mut out);
    pb_bytes(2, header_bytes, &mut out);
    pb_bytes(3, encryption_header_bytes, &mut out);
    pb_bytes(4, body_bytes, &mut out);
    pb_varint(5, u64::from(signature_algorithm), &mut out);
    pb_bytes(6, sender_pubkey, &mut out);
    pb_bytes(7, signature, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Pillar 4 protobuf input models (`pf.nft.metadata.v1` & `pf.nft.claim.v1`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpfsCidRefInput {
    pub cid_bin_hex: String,
    pub byte_length: u64,
    #[serde(default)]
    pub multibase_cid: String,
}

impl IpfsCidRefInput {
    fn encode_proto(&self) -> Result<Vec<u8>, String> {
        let cid_bin =
            hex_to_bytes(&self.cid_bin_hex).map_err(|e| format!("invalid cid_bin_hex: {e}"))?;
        if cid_bin.len() != 36 {
            return Err("cid_bin_hex must decode to 36 bytes".to_string());
        }
        let mut out = Vec::new();
        pb_bytes(1, &cid_bin, &mut out);
        pb_varint(2, self.byte_length, &mut out);
        pb_string(3, &self.multibase_cid, &mut out);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftMediaAttachmentInput {
    pub uri: String,
    #[serde(default)]
    pub cid: Option<IpfsCidRefInput>,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub role: u32,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub byte_length: u64,
    #[serde(default)]
    pub sha3_384_digest_hex: String,
    #[serde(default)]
    pub encrypted: bool,
}

impl NftMediaAttachmentInput {
    fn encode_proto(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        pb_string(1, &self.uri, &mut out);
        if let Some(cid) = &self.cid {
            pb_bytes(2, &cid.encode_proto()?, &mut out);
        }
        pb_string(3, &self.mime_type, &mut out);
        pb_varint(4, u64::from(self.role), &mut out);
        pb_varint(5, u64::from(self.width), &mut out);
        pb_varint(6, u64::from(self.height), &mut out);
        pb_varint(7, self.byte_length, &mut out);
        if !self.sha3_384_digest_hex.is_empty() {
            let digest = hex_to_bytes(&self.sha3_384_digest_hex)
                .map_err(|e| format!("invalid media sha3_384_digest_hex: {e}"))?;
            if digest.len() != 48 {
                return Err("media sha3_384_digest_hex must decode to 48 bytes".to_string());
            }
            pb_bytes(8, &digest, &mut out);
        }
        pb_bool(9, self.encrypted, &mut out);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftAttributeInput {
    pub trait_type: String,
    pub value: String,
    #[serde(default)]
    pub display_type: String,
}

impl NftAttributeInput {
    fn encode_proto(&self) -> Vec<u8> {
        let mut out = Vec::new();
        pb_string(1, &self.trait_type, &mut out);
        pb_string(2, &self.value, &mut out);
        pb_string(3, &self.display_type, &mut out);
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftCollectionRefInput {
    pub collection_id: String,
    pub issuer_account: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub external_url: String,
}

impl NftCollectionRefInput {
    fn encode_proto(&self) -> Vec<u8> {
        let mut out = Vec::new();
        pb_string(1, &self.collection_id, &mut out);
        pb_string(2, &self.issuer_account, &mut out);
        pb_string(3, &self.name, &mut out);
        pb_string(4, &self.description, &mut out);
        pb_string(5, &self.external_url, &mut out);
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftContentHashInput {
    pub uri: String,
    pub algorithm: u32,
    pub digest_hex: String,
    #[serde(default)]
    pub byte_length: u64,
}

impl NftContentHashInput {
    fn encode_proto(&self) -> Result<Vec<u8>, String> {
        let digest = hex_to_bytes(&self.digest_hex)
            .map_err(|e| format!("invalid content_hash digest_hex: {e}"))?;
        let mut out = Vec::new();
        pb_string(1, &self.uri, &mut out);
        pb_varint(2, u64::from(self.algorithm), &mut out);
        pb_bytes(3, &digest, &mut out);
        pb_varint(4, self.byte_length, &mut out);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftOriginProvenanceInput {
    pub source_chain: String,
    pub nftoken_id: String,
    #[serde(default)]
    pub original_issuer: String,
    #[serde(default)]
    pub original_owner: String,
    #[serde(default)]
    pub original_uri: String,
    #[serde(default)]
    pub mint_tx: String,
    #[serde(default)]
    pub mint_ledger_index: u64,
    #[serde(default)]
    pub retirement_mode: u32,
    #[serde(default)]
    pub burn_tx: String,
    #[serde(default)]
    pub burn_ledger_index: u64,
    #[serde(default)]
    pub claim_cid: Option<IpfsCidRefInput>,
    #[serde(default)]
    pub claim_hash_hex: String,
    #[serde(default)]
    pub burn_proof_bundle_cid: Option<IpfsCidRefInput>,
    #[serde(default)]
    pub migrated_serial: u64,
}

impl NftOriginProvenanceInput {
    fn encode_proto(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        pb_string(1, &self.source_chain, &mut out);
        pb_string(2, &self.nftoken_id, &mut out);
        pb_string(3, &self.original_issuer, &mut out);
        pb_string(4, &self.original_owner, &mut out);
        pb_string(5, &self.original_uri, &mut out);
        pb_string(6, &self.mint_tx, &mut out);
        pb_varint(7, self.mint_ledger_index, &mut out);
        pb_varint(8, u64::from(self.retirement_mode), &mut out);
        pb_string(9, &self.burn_tx, &mut out);
        pb_varint(10, self.burn_ledger_index, &mut out);
        if let Some(claim_cid) = &self.claim_cid {
            pb_bytes(11, &claim_cid.encode_proto()?, &mut out);
        }
        if !self.claim_hash_hex.is_empty() {
            let hash = hex_to_bytes(&self.claim_hash_hex)
                .map_err(|e| format!("invalid origin claim_hash_hex: {e}"))?;
            if hash.len() != 32 {
                return Err("origin claim_hash_hex must decode to 32 bytes".to_string());
            }
            pb_bytes(12, &hash, &mut out);
        }
        if let Some(bundle_cid) = &self.burn_proof_bundle_cid {
            pb_bytes(13, &bundle_cid.encode_proto()?, &mut out);
        }
        pb_varint(14, self.migrated_serial, &mut out);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftMetadataInput {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub image_cid: Option<IpfsCidRefInput>,
    #[serde(default)]
    pub media: Vec<NftMediaAttachmentInput>,
    #[serde(default)]
    pub attributes: Vec<NftAttributeInput>,
    #[serde(default)]
    pub collection: Option<NftCollectionRefInput>,
    #[serde(default)]
    pub content_hashes: Vec<NftContentHashInput>,
    #[serde(default)]
    pub prompt_digest_hex: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub external_url: String,
    #[serde(default)]
    pub origin: Option<NftOriginProvenanceInput>,
    #[serde(default)]
    pub extensions: BTreeMap<String, String>,
}

fn default_schema_version() -> u32 {
    1
}

pub fn encode_nft_metadata_proto(metadata: &NftMetadataInput) -> Result<Vec<u8>, String> {
    if metadata.schema_version != 1 {
        return Err("pf.nft.metadata.v1 requires schema_version == 1".to_string());
    }
    if metadata.name.trim().is_empty() {
        return Err("pf.nft.metadata.v1 requires non-empty name".to_string());
    }
    let mut out = Vec::new();
    pb_varint(1, u64::from(metadata.schema_version), &mut out);
    pb_string(2, &metadata.name, &mut out);
    pb_string(3, &metadata.description, &mut out);
    pb_string(4, &metadata.image, &mut out);
    if let Some(image_cid) = &metadata.image_cid {
        pb_bytes(5, &image_cid.encode_proto()?, &mut out);
    }
    for media in &metadata.media {
        pb_bytes(6, &media.encode_proto()?, &mut out);
    }
    for attr in &metadata.attributes {
        pb_bytes(7, &attr.encode_proto(), &mut out);
    }
    if let Some(collection) = &metadata.collection {
        pb_bytes(8, &collection.encode_proto(), &mut out);
    }
    for ch in &metadata.content_hashes {
        pb_bytes(9, &ch.encode_proto()?, &mut out);
    }
    if !metadata.prompt_digest_hex.is_empty() {
        let digest = hex_to_bytes(&metadata.prompt_digest_hex)
            .map_err(|e| format!("invalid prompt_digest_hex: {e}"))?;
        if digest.len() != 48 {
            return Err("prompt_digest_hex must decode to 48 bytes".to_string());
        }
        pb_bytes(10, &digest, &mut out);
    }
    pb_string(11, &metadata.license, &mut out);
    pb_string(12, &metadata.external_url, &mut out);
    if let Some(origin) = &metadata.origin {
        pb_bytes(13, &origin.encode_proto()?, &mut out);
    }
    pb_map_ss(14, &metadata.extensions, &mut out);
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftMigrationClaimInput {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub target_chain_id: String,
    pub source_chain: String,
    pub nftoken_id: String,
    pub old_owner: String,
    pub new_owner_account: String,
    pub target_collection_id: String,
    pub migration_issuer_account: String,
    #[serde(default)]
    pub migrated_serial: u64,
    #[serde(default = "default_retirement_mode")]
    pub retirement_mode: u32,
    pub cutoff_ledger_index: u64,
    pub claimed_at_unix_ms: u64,
    #[serde(default)]
    pub allowlist_cid: Option<IpfsCidRefInput>,
}

fn default_retirement_mode() -> u32 {
    1 // RETIREMENT_MODE_BURN_FIRST
}

pub fn encode_nft_claim_proto(claim: &NftMigrationClaimInput) -> Result<Vec<u8>, String> {
    if claim.schema_version != 1 {
        return Err("pf.nft.claim.v1 requires schema_version == 1".to_string());
    }
    if claim.target_chain_id.is_empty()
        || claim.source_chain.is_empty()
        || claim.nftoken_id.is_empty()
        || claim.old_owner.is_empty()
        || claim.new_owner_account.is_empty()
        || claim.target_collection_id.is_empty()
        || claim.migration_issuer_account.is_empty()
    {
        return Err("pf.nft.claim.v1 has empty required fields".to_string());
    }
    let expected_serial = compute_nft_migrated_serial(&claim.source_chain, &claim.nftoken_id)?;
    let serial = if claim.migrated_serial == 0 {
        expected_serial
    } else {
        if claim.migrated_serial != expected_serial {
            return Err("pf.nft.claim.v1 migrated_serial does not match deterministic hash".into());
        }
        claim.migrated_serial
    };
    if !matches!(claim.retirement_mode, 1 | 2) {
        return Err(
            "pf.nft.claim.v1 retirement_mode must be 1 (BURN_FIRST) or 2 (PIN_ONLY)".into(),
        );
    }
    let mut out = Vec::new();
    pb_varint(1, u64::from(claim.schema_version), &mut out);
    pb_string(2, &claim.target_chain_id, &mut out);
    pb_string(3, &claim.source_chain, &mut out);
    pb_string(4, &claim.nftoken_id, &mut out);
    pb_string(5, &claim.old_owner, &mut out);
    pb_string(6, &claim.new_owner_account, &mut out);
    pb_string(7, &claim.target_collection_id, &mut out);
    pb_string(8, &claim.migration_issuer_account, &mut out);
    pb_varint(9, serial, &mut out);
    pb_varint(10, u64::from(claim.retirement_mode), &mut out);
    pb_varint(11, claim.cutoff_ledger_index, &mut out);
    pb_varint(12, claim.claimed_at_unix_ms, &mut out);
    if let Some(allowlist_cid) = &claim.allowlist_cid {
        pb_bytes(13, &allowlist_cid.encode_proto()?, &mut out);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Signed Pillar 4 envelopes + zero-re-serialization verifier
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNftMetadataEnvelope {
    pub schema_name: String,
    pub sender_account: String,
    pub algorithm_id: String,
    pub sender_pubkey_hex: String,
    pub signature_hex: String,
    pub header_hex: String,
    pub body_hex: String,
    pub envelope_hex: String,
    pub envelope_byte_length: usize,
    pub cid_bin_hex: String,
    pub cid_multibase: String,
    pub metadata_uri: String,
    pub metadata_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNftClaimEnvelope {
    pub schema_name: String,
    pub claimant_account: String,
    pub algorithm_id: String,
    pub sender_pubkey_hex: String,
    pub signature_hex: String,
    pub migrated_serial: u64,
    pub header_hex: String,
    pub body_hex: String,
    pub envelope_hex: String,
    pub envelope_byte_length: usize,
    pub claim_cid_bin_hex: String,
    pub claim_cid_multibase: String,
    pub claim_hash_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedNftEnvelopeSummary {
    pub valid: bool,
    pub schema_name: String,
    pub sender_account: String,
    pub cid_multibase: String,
    pub metadata_hash_sha3_384_hex: String,
    pub claim_hash_sha256_hex: String,
    pub body_byte_length: usize,
}

pub fn sign_nft_metadata_envelope(
    backup: &WalletBackupFile,
    metadata: &NftMetadataInput,
    created_at_unix_ms: u64,
) -> Result<SignedNftMetadataEnvelope, String> {
    let identity = wallet_identity_from_backup(backup).map_err(|e| e.to_string())?;
    if let Some(collection) = &metadata.collection {
        if collection.issuer_account != identity.address {
            return Err(format!(
                "collection.issuer_account `{}` does not match wallet address `{}`",
                collection.issuer_account, identity.address
            ));
        }
    }
    let body_bytes = encode_nft_metadata_proto(metadata)?;
    let header_bytes = encode_public_message_header(
        &backup.chain_id,
        NFT_METADATA_SCHEMA_NAME,
        NFT_METADATA_SCHEMA_ID,
        CONTENT_KIND_NFT,
        &identity.address,
        created_at_unix_ms,
    );
    let preimage = envelope_signing_preimage(1, &header_bytes, &[], &body_bytes);
    let kp = derive_wallet_key_pair(backup).map_err(|e| e.to_string())?;
    let signature = ml_dsa_65_sign_with_context(&kp.private_key, &preimage, DOMAIN_ENVELOPE_SIGN)
        .map_err(|e| format!("ML-DSA-65 envelope sign failed: {e}"))?;
    if !ml_dsa_65_verify_with_context(&kp.public_key, &preimage, &signature, DOMAIN_ENVELOPE_SIGN) {
        return Err("ML-DSA-65 envelope self-verification failed".to_string());
    }
    let envelope_bytes = encode_envelope(
        1,
        &header_bytes,
        &[],
        &body_bytes,
        SIGNATURE_ALGORITHM_ML_DSA_65,
        &kp.public_key,
        &signature,
    );
    if envelope_bytes.len() > MAX_SINGLE_ENVELOPE_BYTES {
        return Err("signed NFT metadata envelope exceeds 1 MiB limit".to_string());
    }
    let (cid_bin, cid_multibase) = cidv1_raw_from_bytes(&envelope_bytes);
    let metadata_uri = format!("ipfs://{cid_multibase}");
    if metadata_uri.len() > MAX_NFT_METADATA_URI_BYTES {
        return Err("metadata_uri exceeds MAX_NFT_METADATA_URI_BYTES".to_string());
    }
    let metadata_hash = sha3_384_hex(&envelope_bytes);
    Ok(SignedNftMetadataEnvelope {
        schema_name: NFT_METADATA_SCHEMA_NAME.to_string(),
        sender_account: identity.address,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        sender_pubkey_hex: bytes_to_hex(&kp.public_key),
        signature_hex: bytes_to_hex(&signature),
        header_hex: bytes_to_hex(&header_bytes),
        body_hex: bytes_to_hex(&body_bytes),
        envelope_byte_length: envelope_bytes.len(),
        envelope_hex: bytes_to_hex(&envelope_bytes),
        cid_bin_hex: bytes_to_hex(&cid_bin),
        cid_multibase,
        metadata_uri,
        metadata_hash,
    })
}

pub fn sign_nft_claim_envelope(
    backup: &WalletBackupFile,
    claim: &NftMigrationClaimInput,
    created_at_unix_ms: u64,
) -> Result<SignedNftClaimEnvelope, String> {
    let identity = wallet_identity_from_backup(backup).map_err(|e| e.to_string())?;
    if claim.target_chain_id != backup.chain_id {
        return Err(format!(
            "claim.target_chain_id `{}` does not match wallet backup chain_id `{}`",
            claim.target_chain_id, backup.chain_id
        ));
    }
    if claim.new_owner_account != identity.address {
        return Err(format!(
            "claim.new_owner_account `{}` does not match wallet address `{}`",
            claim.new_owner_account, identity.address
        ));
    }
    let migrated_serial = compute_nft_migrated_serial(&claim.source_chain, &claim.nftoken_id)?;
    let body_bytes = encode_nft_claim_proto(claim)?;
    let header_bytes = encode_public_message_header(
        &backup.chain_id,
        NFT_CLAIM_SCHEMA_NAME,
        NFT_CLAIM_SCHEMA_ID,
        CONTENT_KIND_NFT,
        &identity.address,
        created_at_unix_ms,
    );
    let preimage = envelope_signing_preimage(1, &header_bytes, &[], &body_bytes);
    let kp = derive_wallet_key_pair(backup).map_err(|e| e.to_string())?;
    let signature = ml_dsa_65_sign_with_context(&kp.private_key, &preimage, DOMAIN_ENVELOPE_SIGN)
        .map_err(|e| format!("ML-DSA-65 claim sign failed: {e}"))?;
    if !ml_dsa_65_verify_with_context(&kp.public_key, &preimage, &signature, DOMAIN_ENVELOPE_SIGN) {
        return Err("ML-DSA-65 claim self-verification failed".to_string());
    }
    let envelope_bytes = encode_envelope(
        1,
        &header_bytes,
        &[],
        &body_bytes,
        SIGNATURE_ALGORITHM_ML_DSA_65,
        &kp.public_key,
        &signature,
    );
    if envelope_bytes.len() > MAX_SINGLE_ENVELOPE_BYTES {
        return Err("signed NFT claim envelope exceeds 1 MiB limit".to_string());
    }
    let (cid_bin, cid_multibase) = cidv1_raw_from_bytes(&envelope_bytes);
    let claim_hash_hex = sha256_hex(&envelope_bytes);
    Ok(SignedNftClaimEnvelope {
        schema_name: NFT_CLAIM_SCHEMA_NAME.to_string(),
        claimant_account: identity.address,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        sender_pubkey_hex: bytes_to_hex(&kp.public_key),
        signature_hex: bytes_to_hex(&signature),
        migrated_serial,
        header_hex: bytes_to_hex(&header_bytes),
        body_hex: bytes_to_hex(&body_bytes),
        envelope_byte_length: envelope_bytes.len(),
        envelope_hex: bytes_to_hex(&envelope_bytes),
        claim_cid_bin_hex: bytes_to_hex(&cid_bin),
        claim_cid_multibase: cid_multibase,
        claim_hash_hex,
    })
}

/// Verify a signed PUBLIC Pillar 2/4 `Envelope` strictly over the raw transmitted bytes
/// (zero re-serialization rule, Contract C1 §2.2 & §6.1).
pub fn verify_signed_public_nft_envelope(
    envelope_bytes: &[u8],
    expected_schema_name: &str,
    expected_sender_account: &str,
    expected_metadata_hash_sha3_384: Option<&str>,
) -> Result<VerifiedNftEnvelopeSummary, String> {
    if envelope_bytes.is_empty() || envelope_bytes.len() > MAX_SINGLE_ENVELOPE_BYTES {
        return Err("envelope size out of bounds".to_string());
    }
    let actual_metadata_hash = sha3_384_hex(envelope_bytes);
    if let Some(expected_hash) = expected_metadata_hash_sha3_384 {
        if actual_metadata_hash != expected_hash {
            return Err(
                "envelope SHA3-384 digest does not match expected metadata_hash".to_string(),
            );
        }
    }
    let fields = decode_wire_fields(envelope_bytes)?;
    let mut version = 0u32;
    let mut header_bytes: &[u8] = &[];
    let mut enc_header_bytes: &[u8] = &[];
    let mut body_bytes: &[u8] = &[];
    let mut sig_alg = 0u32;
    let mut sender_pubkey: &[u8] = &[];
    let mut signature: &[u8] = &[];
    for (num, value) in fields {
        match (num, value) {
            (1, WireValue::Varint(v)) => {
                version = u32::try_from(v).map_err(|_| "envelope version overflow".to_string())?;
            }
            (2, WireValue::Bytes(b)) => header_bytes = b,
            (3, WireValue::Bytes(b)) => enc_header_bytes = b,
            (4, WireValue::Bytes(b)) => body_bytes = b,
            (5, WireValue::Varint(v)) => {
                sig_alg = u32::try_from(v).map_err(|_| "sig alg overflow".to_string())?;
            }
            (6, WireValue::Bytes(b)) => sender_pubkey = b,
            (7, WireValue::Bytes(b)) => signature = b,
            _ => {}
        }
    }
    if version != 1 {
        return Err("unsupported envelope version".to_string());
    }
    if !enc_header_bytes.is_empty() {
        return Err("public NFT envelope must not carry encryption_header_bytes".to_string());
    }
    if sig_alg != SIGNATURE_ALGORITHM_ML_DSA_65 {
        return Err("envelope must use ML-DSA-65 signature algorithm".to_string());
    }
    let derived_sender = address_from_public_key(sender_pubkey);
    if derived_sender != expected_sender_account {
        return Err(format!(
            "envelope sender `{derived_sender}` does not match expected `{expected_sender_account}`"
        ));
    }
    let preimage = envelope_signing_preimage(version, header_bytes, enc_header_bytes, body_bytes);
    if !ml_dsa_65_verify_with_context(sender_pubkey, &preimage, signature, DOMAIN_ENVELOPE_SIGN) {
        return Err("envelope ML-DSA-65 signature verification failed".to_string());
    }

    // Verify header schema_name and sender_account AFTER signature verification succeeds.
    let mut header_schema = String::new();
    let mut header_sender = String::new();
    let mut header_visibility = 0u32;
    for (num, value) in decode_wire_fields(header_bytes)? {
        match (num, value) {
            (3, WireValue::Bytes(b)) => {
                header_schema = std::str::from_utf8(b)
                    .map_err(|_| "invalid utf8 in header.schema_name".to_string())?
                    .to_string();
            }
            (6, WireValue::Varint(v)) => {
                header_visibility =
                    u32::try_from(v).map_err(|_| "header visibility overflow".to_string())?;
            }
            (9, WireValue::Bytes(b)) => {
                header_sender = std::str::from_utf8(b)
                    .map_err(|_| "invalid utf8 in header.sender_account".to_string())?
                    .to_string();
            }
            _ => {}
        }
    }
    if header_schema != expected_schema_name {
        return Err(format!(
            "envelope schema_name `{header_schema}` does not match expected `{expected_schema_name}`"
        ));
    }
    if header_visibility != VISIBILITY_PUBLIC {
        return Err("envelope header must declare VISIBILITY_PUBLIC".to_string());
    }
    if header_sender != derived_sender {
        return Err("envelope header.sender_account does not match signer pubkey".to_string());
    }

    let (_, cid_multibase) = cidv1_raw_from_bytes(envelope_bytes);
    Ok(VerifiedNftEnvelopeSummary {
        valid: true,
        schema_name: header_schema,
        sender_account: derived_sender,
        cid_multibase,
        metadata_hash_sha3_384_hex: actual_metadata_hash,
        claim_hash_sha256_hex: sha256_hex(envelope_bytes),
        body_byte_length: body_bytes.len(),
    })
}

// ---------------------------------------------------------------------------
// On-chain `nft_mint`, `nft_transfer`, `nft_burn` builders & ML-DSA-65 signers
// ---------------------------------------------------------------------------

/// Build and validate an `nft_mint` operation (including Pillar 4 unsolicited permanent-lock guard).
pub fn build_nft_mint_operation(op: NftMintOperation) -> Result<NftMintOperation, String> {
    op.validate()?;
    let is_burn_locked = op.collection_flags & NFT_COLLECTION_FLAG_BURN_LOCKED != 0;
    let is_non_transferable = (op.collection_flags & NFT_COLLECTION_FLAG_TRANSFER_LOCKED != 0)
        || (op.flags & NFT_FLAG_TRANSFERABLE == 0);
    if op.owner != op.issuer && is_burn_locked && is_non_transferable {
        return Err(
            "nft_unsolicited_permanent_lock_forbidden: unsolicited mint to another account cannot be both burn-locked and non-transferable"
                .to_string(),
        );
    }
    Ok(op)
}

/// Build and validate an `nft_transfer` operation.
pub fn build_nft_transfer_operation(
    op: NftTransferOperation,
) -> Result<NftTransferOperation, String> {
    op.validate()?;
    Ok(op)
}

/// Build and validate an owner-initiated `nft_burn` operation.
pub fn build_nft_burn_operation(op: NftBurnOperation) -> Result<NftBurnOperation, String> {
    op.validate()?;
    Ok(op)
}

/// Train A `ISSUER_BURNABLE` issuer-initiated burn payload (ready once Train A activates).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainANftBurnPayload {
    pub operation: String,
    pub nft_id: String,
    pub owner: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub issuer: String,
}

pub fn build_train_a_nft_burn_payload(
    nft_id_hex: &str,
    owner: &str,
    issuer: Option<&str>,
) -> Result<TrainANftBurnPayload, String> {
    let base = NftBurnOperation {
        nft_id: nft_id_hex.to_string(),
        owner: owner.to_string(),
    };
    base.validate()?;
    let issuer_str = issuer.unwrap_or_default().to_string();
    if !issuer_str.is_empty() && issuer_str.trim() != issuer_str {
        return Err("nft_burn.issuer must not contain leading/trailing whitespace".to_string());
    }
    Ok(TrainANftBurnPayload {
        operation: "nft_burn".to_string(),
        nft_id: base.nft_id,
        owner: base.owner,
        issuer: issuer_str,
    })
}

pub fn compute_onchain_nft_id(
    chain_id: &str,
    issuer: &str,
    collection_id: &str,
    serial: u64,
) -> Result<String, String> {
    nft_id(chain_id, issuer, collection_id, serial)
}

/// Sign an on-chain NFT transaction (`nft_mint`, `nft_transfer`, `nft_burn`) from an RPC quote.
pub fn sign_nft_transaction_quote(
    backup: &WalletBackupFile,
    quote_response: &RpcResponse,
) -> Result<SignedNftTransaction, String> {
    if let Some(op_val) = quote_response
        .result
        .as_ref()
        .and_then(|r| r.get("operation"))
    {
        if let Ok(postfiat_types::NftTransactionOperation::NftMint(mint)) =
            serde_json::from_value::<postfiat_types::NftTransactionOperation>(op_val.clone())
        {
            build_nft_mint_operation(mint)?;
        }
    }
    wallet_sign_nft_transaction_from_quote(backup, quote_response).map_err(|e| e.to_string())
}

/// Sign an on-chain NFT transaction (`nft_mint`, `nft_transfer`, `nft_burn`) from explicit fields.
pub fn sign_nft_transaction_fields(
    backup: &WalletBackupFile,
    fields: WalletSignNftTransactionFields,
) -> Result<SignedNftTransaction, String> {
    if let postfiat_types::NftTransactionOperation::NftMint(ref mint) = fields.operation {
        build_nft_mint_operation(mint.clone())?;
    }
    wallet_sign_nft_transaction_from_fields(backup, fields).map_err(|e| e.to_string())
}
