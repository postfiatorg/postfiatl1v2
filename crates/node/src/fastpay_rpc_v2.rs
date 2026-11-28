//! Compact Protobuf v3 FastPay RPC v2 (`postfiat-fastpay-rpc-v2`) over direct TLS 1.3 TCP.
//!
//! Framing: `[u8 version = 2][u32be frame_len][protobuf payload]`
//! - Hard pre-allocation cap: `64 KiB` (`MAX_FASTPAY_V2_FRAME_BYTES`)
//! - `owned_sign_v3` (~7.5 KB with `owner_is_change_recipient` flag) fits inside 14.6 KB initcwnd
//! - `owned_apply_v3_fast` (~16.7 KB votes-only via 256-entry order LRU, transparent `OrderNotCached` fallback)
//! - Direct TLS 1.3 (`rustls` + `ring`), optional mTLS client CA verification, `TCP_NODELAY = 1`,
//!   per-IP token-bucket rate limiter, and max concurrent connection cap.

use crate::{
    owned_apply_v3_fast_typed, owned_apply_v3_typed, owned_sign_v3_typed,
    prewarm_fastpay_ledger_cache_from_sidecar, take_fastpay_rpc_metrics, FastPayApplyOutcomeV3,
    NodeOptions,
};
use postfiat_types::{
    FastPayApplyAckV1, FastPayOrderRecoveryV1, OwnedCertificateDomain, OwnedObjectRef,
    OwnedOutputSpec, OwnedTransferCertificateV3, OwnedTransferOrderV3, OwnedTransferVote,
    PaymentMemo, SignedOwnedTransferOrderV3,
};
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig, ServerConnection, StreamOwned};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const FASTPAY_RPC_V2_VERSION: u8 = 2;
pub const MAX_FASTPAY_V2_FRAME_BYTES: usize = 65_536;
pub const MAX_FASTPAY_V2_CONCURRENT_CONNECTIONS: usize = 128;
pub const DEFAULT_FASTPAY_V2_RATE_LIMIT_PER_SEC: f64 = 200.0;
pub const DEFAULT_FASTPAY_V2_RATE_BURST: f64 = 400.0;

pub const METHOD_OWNED_SIGN_V3: u32 = 1;
pub const METHOD_OWNED_APPLY_V3_FAST: u32 = 2;
pub const METHOD_OWNED_APPLY_V3_FULL: u32 = 3;

pub const STATUS_OK: u32 = 0;
pub const STATUS_INVALID_INPUT: u32 = 1;
pub const STATUS_UNSUPPORTED_PROTOCOL_VERSION: u32 = 2;
pub const STATUS_ORDER_NOT_CACHED: u32 = 3;
pub const STATUS_RATE_LIMITED: u32 = 4;
pub const STATUS_INTERNAL_ERROR: u32 = 5;

const ML_DSA_65_PUBKEY_BYTES: usize = 1952;
const ML_DSA_65_SIG_BYTES: usize = 3309;
const HASH_48_BYTES: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastPayRpcV2DecodeError {
    UnsupportedProtocolVersion(u8),
    FrameTooLarge(usize),
    Malformed(String),
}

impl std::fmt::Display for FastPayRpcV2DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion(v) => {
                write!(
                    f,
                    "UnsupportedProtocolVersion: got {v}, expected {FASTPAY_RPC_V2_VERSION}"
                )
            }
            Self::FrameTooLarge(len) => {
                write!(
                    f,
                    "frame length {len} exceeds cap {MAX_FASTPAY_V2_FRAME_BYTES}"
                )
            }
            Self::Malformed(msg) => write!(f, "malformed protobuf v2 frame: {msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastPayRpcV2MethodPayload {
    OwnedSignV3(SignedOwnedTransferOrderV3),
    OwnedApplyV3Fast {
        lock_id: String,
        votes: Vec<OwnedTransferVote>,
    },
    OwnedApplyV3Full(OwnedTransferCertificateV3),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastPayRpcV2Request {
    pub request_id: u64,
    pub validator_id: String,
    pub payload: FastPayRpcV2MethodPayload,
}

#[derive(Debug, Clone)]
pub struct FastPayRpcV2Response {
    pub request_id: u64,
    pub status: u32,
    pub error_message: String,
    pub vote: Option<OwnedTransferVote>,
    pub apply_outcome: Option<FastPayApplyOutcomeV3>,
}

// ---------------------------------------------------------------------------
// Canonical Protobuf v3 Wire Primitives (zero external dependencies)
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

enum WireValue<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

fn decode_wire_fields(buf: &[u8]) -> Result<Vec<(u32, WireValue<'_>)>, String> {
    let mut fields = Vec::new();
    let mut offset = 0usize;
    while offset < buf.len() {
        let tag = decode_varint(buf, &mut offset)?;
        let field_num =
            u32::try_from(tag >> 3).map_err(|_| "protobuf field number overflow".to_string())?;
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
                    .map_err(|_| "protobuf length overflow".to_string())?;
                if offset.checked_add(len).is_none_or(|end| end > buf.len()) {
                    return Err("truncated protobuf length-delimited field".to_string());
                }
                fields.push((field_num, WireValue::Bytes(&buf[offset..offset + len])));
                offset += len;
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

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_to_raw(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("odd hex length".to_string());
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex byte {b:#x}")),
    }
}

fn wire_str<'a>(val: &WireValue<'a>, label: &str) -> Result<&'a str, String> {
    match val {
        WireValue::Bytes(b) => {
            std::str::from_utf8(b).map_err(|_| format!("{label}: invalid utf-8"))
        }
        _ => Err(format!("{label}: expected bytes/string")),
    }
}

fn wire_bytes<'a>(val: &WireValue<'a>, label: &str) -> Result<&'a [u8], String> {
    match val {
        WireValue::Bytes(b) => Ok(*b),
        _ => Err(format!("{label}: expected bytes")),
    }
}

fn wire_u64(val: &WireValue<'_>, label: &str) -> Result<u64, String> {
    match val {
        WireValue::Varint(v) => Ok(*v),
        _ => Err(format!("{label}: expected varint")),
    }
}

// ---------------------------------------------------------------------------
// Protobuf Message Encoders & Decoders
// ---------------------------------------------------------------------------

fn encode_domain(domain: &OwnedCertificateDomain) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(192);
    pb_string(1, &domain.schema, &mut out);
    pb_string(2, &domain.chain_id, &mut out);
    pb_bytes(3, &hex_to_raw(&domain.genesis_hash)?, &mut out);
    pb_varint(4, u64::from(domain.protocol_version), &mut out);
    pb_bytes(5, &hex_to_raw(&domain.registry_id)?, &mut out);
    Ok(out)
}

fn decode_domain(buf: &[u8]) -> Result<OwnedCertificateDomain, String> {
    let mut schema = String::new();
    let mut chain_id = String::new();
    let mut genesis_hash = String::new();
    let mut protocol_version = 0u32;
    let mut registry_id = String::new();
    for (num, val) in decode_wire_fields(buf)? {
        match num {
            1 => schema = wire_str(&val, "domain.schema")?.to_string(),
            2 => chain_id = wire_str(&val, "domain.chain_id")?.to_string(),
            3 => {
                let raw = wire_bytes(&val, "domain.genesis_hash")?;
                if raw.len() != HASH_48_BYTES {
                    return Err(format!("domain.genesis_hash must be {HASH_48_BYTES} bytes"));
                }
                genesis_hash = bytes_to_lower_hex(raw);
            }
            4 => {
                protocol_version = u32::try_from(wire_u64(&val, "domain.protocol_version")?)
                    .map_err(|_| "domain.protocol_version overflow".to_string())?;
            }
            5 => {
                let raw = wire_bytes(&val, "domain.registry_id")?;
                if raw.len() != HASH_48_BYTES {
                    return Err(format!("domain.registry_id must be {HASH_48_BYTES} bytes"));
                }
                registry_id = bytes_to_lower_hex(raw);
            }
            _ => {}
        }
    }
    Ok(OwnedCertificateDomain {
        schema,
        chain_id,
        genesis_hash,
        protocol_version,
        registry_id,
    })
}

fn encode_recovery(rec: &FastPayOrderRecoveryV1) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(128);
    pb_string(1, &rec.schema, &mut out);
    pb_varint(2, rec.committee_epoch, &mut out);
    pb_varint(3, rec.valid_from_height, &mut out);
    pb_varint(4, rec.expires_at_height, &mut out);
    pb_varint(5, rec.recovery_closes_at_height, &mut out);
    pb_bytes(6, &hex_to_raw(&rec.lock_id)?, &mut out);
    Ok(out)
}

fn decode_recovery(buf: &[u8]) -> Result<FastPayOrderRecoveryV1, String> {
    let mut schema = String::new();
    let mut committee_epoch = 0u64;
    let mut valid_from_height = 0u64;
    let mut expires_at_height = 0u64;
    let mut recovery_closes_at_height = 0u64;
    let mut lock_id = String::new();
    for (num, val) in decode_wire_fields(buf)? {
        match num {
            1 => schema = wire_str(&val, "recovery.schema")?.to_string(),
            2 => committee_epoch = wire_u64(&val, "recovery.committee_epoch")?,
            3 => valid_from_height = wire_u64(&val, "recovery.valid_from_height")?,
            4 => expires_at_height = wire_u64(&val, "recovery.expires_at_height")?,
            5 => recovery_closes_at_height = wire_u64(&val, "recovery.recovery_closes_at_height")?,
            6 => {
                let raw = wire_bytes(&val, "recovery.lock_id")?;
                if raw.len() != HASH_48_BYTES {
                    return Err(format!("recovery.lock_id must be {HASH_48_BYTES} bytes"));
                }
                lock_id = bytes_to_lower_hex(raw);
            }
            _ => {}
        }
    }
    Ok(FastPayOrderRecoveryV1 {
        schema,
        committee_epoch,
        valid_from_height,
        expires_at_height,
        recovery_closes_at_height,
        lock_id,
    })
}

fn encode_signed_order(signed: &SignedOwnedTransferOrderV3) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(8192);
    let owner_pk_raw = hex_to_raw(&signed.owner_pubkey_hex)?;
    let owner_sig_raw = hex_to_raw(&signed.owner_signature_hex)?;
    pb_bytes(1, &owner_pk_raw, &mut out);
    pb_bytes(2, &owner_sig_raw, &mut out);
    pb_bytes(3, &encode_domain(&signed.order.domain)?, &mut out);
    pb_bytes(4, &encode_recovery(&signed.order.recovery)?, &mut out);
    for input in &signed.order.inputs {
        let mut enc = Vec::with_capacity(input.id.len() + 16);
        pb_string(1, &input.id, &mut enc);
        pb_varint(2, input.version, &mut enc);
        pb_bytes(5, &enc, &mut out);
    }
    for output in &signed.order.outputs {
        let mut enc = Vec::with_capacity(64);
        let is_owner_change = output
            .owner_pubkey_hex
            .eq_ignore_ascii_case(&signed.owner_pubkey_hex);
        if is_owner_change {
            pb_bool(4, true, &mut enc);
        } else {
            pb_bytes(1, &hex_to_raw(&output.owner_pubkey_hex)?, &mut enc);
        }
        pb_varint(2, output.value, &mut enc);
        pb_string(3, &output.asset, &mut enc);
        pb_bytes(6, &enc, &mut out);
    }
    pb_varint(7, signed.order.fee, &mut out);
    pb_varint(8, signed.order.nonce, &mut out);
    for memo in &signed.order.memos {
        let mut mc = Vec::with_capacity(
            memo.memo_type.len() + memo.memo_format.len() + memo.memo_data.len() + 16,
        );
        pb_string(1, &memo.memo_type, &mut mc);
        pb_string(2, &memo.memo_format, &mut mc);
        pb_string(3, &memo.memo_data, &mut mc);
        pb_bytes(9, &mc, &mut out);
    }
    Ok(out)
}

fn decode_signed_order(buf: &[u8]) -> Result<SignedOwnedTransferOrderV3, String> {
    let mut owner_pubkey_raw: Option<Vec<u8>> = None;
    let mut owner_sig_raw: Option<Vec<u8>> = None;
    let mut domain: Option<OwnedCertificateDomain> = None;
    let mut recovery: Option<FastPayOrderRecoveryV1> = None;
    let mut inputs = Vec::new();
    let mut raw_outputs: Vec<(Option<Vec<u8>>, u64, String, bool)> = Vec::new();
    let mut fee = 0u64;
    let mut nonce = 0u64;
    let mut memos = Vec::new();

    for (num, val) in decode_wire_fields(buf)? {
        match num {
            1 => {
                let raw = wire_bytes(&val, "order.owner_pubkey")?;
                if raw.len() != ML_DSA_65_PUBKEY_BYTES {
                    return Err(format!(
                        "owner_pubkey must be {ML_DSA_65_PUBKEY_BYTES} bytes, got {}",
                        raw.len()
                    ));
                }
                owner_pubkey_raw = Some(raw.to_vec());
            }
            2 => {
                let raw = wire_bytes(&val, "order.owner_signature")?;
                if raw.len() != ML_DSA_65_SIG_BYTES {
                    return Err(format!(
                        "owner_signature must be {ML_DSA_65_SIG_BYTES} bytes, got {}",
                        raw.len()
                    ));
                }
                owner_sig_raw = Some(raw.to_vec());
            }
            3 => domain = Some(decode_domain(wire_bytes(&val, "order.domain")?)?),
            4 => recovery = Some(decode_recovery(wire_bytes(&val, "order.recovery")?)?),
            5 => {
                let mut id = String::new();
                let mut version = 0u64;
                for (f, v) in decode_wire_fields(wire_bytes(&val, "order.input")?)? {
                    match f {
                        1 => {
                            let s = wire_str(&v, "input.id")?;
                            if s.is_empty() || s.len() > 256 {
                                return Err("input.id length out of bounds".to_string());
                            }
                            id = s.to_string();
                        }
                        2 => version = wire_u64(&v, "input.version")?,
                        _ => {}
                    }
                }
                inputs.push(OwnedObjectRef { id, version });
            }
            6 => {
                let mut pk: Option<Vec<u8>> = None;
                let mut value = 0u64;
                let mut asset = String::new();
                let mut owner_is_change = false;
                for (f, v) in decode_wire_fields(wire_bytes(&val, "order.output")?)? {
                    match f {
                        1 => {
                            let raw = wire_bytes(&v, "output.owner_pubkey")?;
                            if raw.len() != ML_DSA_65_PUBKEY_BYTES {
                                return Err(format!(
                                    "output.owner_pubkey must be {ML_DSA_65_PUBKEY_BYTES} bytes"
                                ));
                            }
                            pk = Some(raw.to_vec());
                        }
                        2 => value = wire_u64(&v, "output.value")?,
                        3 => asset = wire_str(&v, "output.asset")?.to_string(),
                        4 => owner_is_change = wire_u64(&v, "output.owner_is_change")? != 0,
                        _ => {}
                    }
                }
                raw_outputs.push((pk, value, asset, owner_is_change));
            }
            7 => fee = wire_u64(&val, "order.fee")?,
            8 => nonce = wire_u64(&val, "order.nonce")?,
            9 => {
                let mut memo_type = String::new();
                let mut memo_format = String::new();
                let mut memo_data = String::new();
                for (f, v) in decode_wire_fields(wire_bytes(&val, "order.memo")?)? {
                    match f {
                        1 => memo_type = wire_str(&v, "memo.memo_type")?.to_string(),
                        2 => memo_format = wire_str(&v, "memo.memo_format")?.to_string(),
                        3 => memo_data = wire_str(&v, "memo.memo_data")?.to_string(),
                        _ => {}
                    }
                }
                memos.push(PaymentMemo {
                    memo_type,
                    memo_format,
                    memo_data,
                });
            }
            _ => {}
        }
    }

    let owner_pk_raw = owner_pubkey_raw.ok_or_else(|| "missing owner_pubkey".to_string())?;
    let owner_sig_raw = owner_sig_raw.ok_or_else(|| "missing owner_signature".to_string())?;
    let owner_pubkey_hex = bytes_to_lower_hex(&owner_pk_raw);
    let owner_signature_hex = bytes_to_lower_hex(&owner_sig_raw);

    let mut outputs = Vec::with_capacity(raw_outputs.len());
    for (pk_opt, value, asset, is_owner_change) in raw_outputs {
        let out_pk_hex = if is_owner_change {
            owner_pubkey_hex.clone()
        } else {
            let pk = pk_opt.ok_or_else(|| "output missing owner_pubkey".to_string())?;
            bytes_to_lower_hex(&pk)
        };
        outputs.push(OwnedOutputSpec {
            owner_pubkey_hex: out_pk_hex,
            value,
            asset,
        });
    }

    Ok(SignedOwnedTransferOrderV3 {
        order: OwnedTransferOrderV3 {
            domain: domain.ok_or_else(|| "missing domain".to_string())?,
            recovery: recovery.ok_or_else(|| "missing recovery".to_string())?,
            inputs,
            outputs,
            fee,
            nonce,
            memos,
        },
        owner_pubkey_hex,
        owner_signature_hex,
    })
}

fn encode_vote(vote: &OwnedTransferVote) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(3340);
    pb_string(1, &vote.validator_id, &mut out);
    pb_bytes(2, &hex_to_raw(&vote.signature_hex)?, &mut out);
    Ok(out)
}

fn decode_vote(buf: &[u8]) -> Result<OwnedTransferVote, String> {
    let mut validator_id = String::new();
    let mut sig_raw: Option<Vec<u8>> = None;
    for (num, val) in decode_wire_fields(buf)? {
        match num {
            1 => validator_id = wire_str(&val, "vote.validator_id")?.to_string(),
            2 => {
                let raw = wire_bytes(&val, "vote.signature")?;
                if raw.len() != ML_DSA_65_SIG_BYTES {
                    return Err(format!(
                        "vote.signature must be {ML_DSA_65_SIG_BYTES} bytes, got {}",
                        raw.len()
                    ));
                }
                sig_raw = Some(raw.to_vec());
            }
            _ => {}
        }
    }
    let sig_raw = sig_raw.ok_or_else(|| "vote missing signature".to_string())?;
    if validator_id.is_empty() {
        return Err("vote missing validator_id".to_string());
    }
    Ok(OwnedTransferVote {
        validator_id,
        signature_hex: bytes_to_lower_hex(&sig_raw),
    })
}

fn encode_apply_ack(ack: &FastPayApplyAckV1) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(3700);
    pb_string(1, &ack.schema, &mut out);
    pb_bytes(2, &encode_domain(&ack.domain)?, &mut out);
    pb_varint(3, ack.committee_epoch, &mut out);
    pb_bytes(4, &hex_to_raw(&ack.lock_id)?, &mut out);
    pb_bytes(5, &hex_to_raw(&ack.order_digest)?, &mut out);
    pb_bytes(6, &hex_to_raw(&ack.certificate_digest)?, &mut out);
    pb_bytes(7, &hex_to_raw(&ack.terminal_state_digest)?, &mut out);
    pb_string(8, &ack.validator_id, &mut out);
    pb_bytes(9, &hex_to_raw(&ack.signature_hex)?, &mut out);
    pb_bool(10, true, &mut out); // signed = true
    Ok(out)
}

fn encode_held_effect(
    validator_id: &str,
    lock_id: &str,
    certificate_digest: &str,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(200);
    pb_string(1, crate::FASTPAY_HELD_EFFECT_SCHEMA_V1, &mut out);
    pb_bytes(4, &hex_to_raw(lock_id)?, &mut out);
    pb_bytes(6, &hex_to_raw(certificate_digest)?, &mut out);
    pb_string(8, validator_id, &mut out);
    // field 10 (signed) omitted => false
    Ok(out)
}

pub fn encode_fastpay_rpc_v2_request_frame(req: &FastPayRpcV2Request) -> Result<Vec<u8>, String> {
    let mut body = Vec::with_capacity(8192);
    pb_varint(1, req.request_id, &mut body);
    match &req.payload {
        FastPayRpcV2MethodPayload::OwnedSignV3(signed) => {
            pb_varint(2, u64::from(METHOD_OWNED_SIGN_V3), &mut body);
            pb_string(3, &req.validator_id, &mut body);
            pb_bytes(4, &encode_signed_order(signed)?, &mut body);
        }
        FastPayRpcV2MethodPayload::OwnedApplyV3Fast { lock_id, votes } => {
            pb_varint(2, u64::from(METHOD_OWNED_APPLY_V3_FAST), &mut body);
            pb_string(3, &req.validator_id, &mut body);
            let mut inner = Vec::with_capacity(17_000);
            pb_bytes(1, &hex_to_raw(lock_id)?, &mut inner);
            for vote in votes {
                pb_bytes(2, &encode_vote(vote)?, &mut inner);
            }
            pb_bytes(5, &inner, &mut body);
        }
        FastPayRpcV2MethodPayload::OwnedApplyV3Full(cert) => {
            pb_varint(2, u64::from(METHOD_OWNED_APPLY_V3_FULL), &mut body);
            pb_string(3, &req.validator_id, &mut body);
            let mut inner = Vec::with_capacity(25_000);
            let signed = SignedOwnedTransferOrderV3 {
                order: cert.order.clone(),
                owner_pubkey_hex: cert.owner_pubkey_hex.clone(),
                owner_signature_hex: cert.owner_signature_hex.clone(),
            };
            pb_bytes(1, &encode_signed_order(&signed)?, &mut inner);
            for vote in &cert.votes {
                pb_bytes(2, &encode_vote(vote)?, &mut inner);
            }
            pb_bytes(6, &inner, &mut body);
        }
    }
    if body.len() > MAX_FASTPAY_V2_FRAME_BYTES {
        return Err(format!(
            "encoded request length {} exceeds cap {MAX_FASTPAY_V2_FRAME_BYTES}",
            body.len()
        ));
    }
    let mut frame = Vec::with_capacity(5 + body.len());
    frame.push(FASTPAY_RPC_V2_VERSION);
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Fuzz-hardened frame decoder: validates version byte, 64 KiB cap, and canonical protobuf3 fields
/// before any signature verification or state access.
pub fn decode_fastpay_rpc_v2_frame(
    frame: &[u8],
) -> Result<FastPayRpcV2Request, FastPayRpcV2DecodeError> {
    if frame.len() < 5 {
        return Err(FastPayRpcV2DecodeError::Malformed(
            "frame shorter than 5-byte header".to_string(),
        ));
    }
    let version = frame[0];
    if version != FASTPAY_RPC_V2_VERSION {
        return Err(FastPayRpcV2DecodeError::UnsupportedProtocolVersion(version));
    }
    let len = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
    if len == 0 || len > MAX_FASTPAY_V2_FRAME_BYTES {
        return Err(FastPayRpcV2DecodeError::FrameTooLarge(len));
    }
    if frame.len() != 5 + len {
        return Err(FastPayRpcV2DecodeError::Malformed(format!(
            "frame body length mismatch: header={len}, actual={}",
            frame.len() - 5
        )));
    }
    decode_fastpay_rpc_v2_body(&frame[5..])
}

pub fn decode_fastpay_rpc_v2_body(
    body: &[u8],
) -> Result<FastPayRpcV2Request, FastPayRpcV2DecodeError> {
    if body.is_empty() || body.len() > MAX_FASTPAY_V2_FRAME_BYTES {
        return Err(FastPayRpcV2DecodeError::FrameTooLarge(body.len()));
    }
    let fields = decode_wire_fields(body).map_err(FastPayRpcV2DecodeError::Malformed)?;
    let mut request_id = 0u64;
    let mut method = 0u32;
    let mut validator_id = String::new();
    let mut sign_bytes: Option<&[u8]> = None;
    let mut apply_fast_bytes: Option<&[u8]> = None;
    let mut apply_full_bytes: Option<&[u8]> = None;

    for (num, val) in fields {
        match num {
            1 => {
                request_id =
                    wire_u64(&val, "request_id").map_err(FastPayRpcV2DecodeError::Malformed)?;
            }
            2 => {
                method = u32::try_from(
                    wire_u64(&val, "method").map_err(FastPayRpcV2DecodeError::Malformed)?,
                )
                .map_err(|_| FastPayRpcV2DecodeError::Malformed("method overflow".to_string()))?;
            }
            3 => {
                validator_id = wire_str(&val, "validator_id")
                    .map_err(FastPayRpcV2DecodeError::Malformed)?
                    .to_string();
            }
            4 => {
                sign_bytes =
                    Some(wire_bytes(&val, "sign_req").map_err(FastPayRpcV2DecodeError::Malformed)?);
            }
            5 => {
                apply_fast_bytes = Some(
                    wire_bytes(&val, "apply_fast_req")
                        .map_err(FastPayRpcV2DecodeError::Malformed)?,
                );
            }
            6 => {
                apply_full_bytes = Some(
                    wire_bytes(&val, "apply_full_req")
                        .map_err(FastPayRpcV2DecodeError::Malformed)?,
                );
            }
            _ => {}
        }
    }

    if validator_id.is_empty() || validator_id.len() > 128 {
        return Err(FastPayRpcV2DecodeError::Malformed(
            "invalid validator_id".to_string(),
        ));
    }

    let payload = match method {
        METHOD_OWNED_SIGN_V3 => {
            let raw = sign_bytes.ok_or_else(|| {
                FastPayRpcV2DecodeError::Malformed("missing sign_req payload".to_string())
            })?;
            let signed = decode_signed_order(raw).map_err(FastPayRpcV2DecodeError::Malformed)?;
            FastPayRpcV2MethodPayload::OwnedSignV3(signed)
        }
        METHOD_OWNED_APPLY_V3_FAST => {
            let raw = apply_fast_bytes.ok_or_else(|| {
                FastPayRpcV2DecodeError::Malformed("missing apply_fast_req payload".to_string())
            })?;
            let mut lock_id = String::new();
            let mut votes = Vec::new();
            for (f, v) in decode_wire_fields(raw).map_err(FastPayRpcV2DecodeError::Malformed)? {
                match f {
                    1 => {
                        let b = wire_bytes(&v, "apply_fast.lock_id")
                            .map_err(FastPayRpcV2DecodeError::Malformed)?;
                        if b.len() != HASH_48_BYTES {
                            return Err(FastPayRpcV2DecodeError::Malformed(format!(
                                "apply_fast.lock_id must be {HASH_48_BYTES} bytes"
                            )));
                        }
                        lock_id = bytes_to_lower_hex(b);
                    }
                    2 => {
                        if votes.len() >= 16 {
                            return Err(FastPayRpcV2DecodeError::Malformed(
                                "too many votes in apply_fast_req".to_string(),
                            ));
                        }
                        votes.push(
                            decode_vote(
                                wire_bytes(&v, "apply_fast.vote")
                                    .map_err(FastPayRpcV2DecodeError::Malformed)?,
                            )
                            .map_err(FastPayRpcV2DecodeError::Malformed)?,
                        );
                    }
                    _ => {}
                }
            }
            if lock_id.is_empty() || votes.is_empty() {
                return Err(FastPayRpcV2DecodeError::Malformed(
                    "apply_fast_req requires lock_id and votes".to_string(),
                ));
            }
            FastPayRpcV2MethodPayload::OwnedApplyV3Fast { lock_id, votes }
        }
        METHOD_OWNED_APPLY_V3_FULL => {
            let raw = apply_full_bytes.ok_or_else(|| {
                FastPayRpcV2DecodeError::Malformed("missing apply_full_req payload".to_string())
            })?;
            let mut signed_opt: Option<SignedOwnedTransferOrderV3> = None;
            let mut votes = Vec::new();
            for (f, v) in decode_wire_fields(raw).map_err(FastPayRpcV2DecodeError::Malformed)? {
                match f {
                    1 => {
                        signed_opt = Some(
                            decode_signed_order(
                                wire_bytes(&v, "apply_full.signed_order")
                                    .map_err(FastPayRpcV2DecodeError::Malformed)?,
                            )
                            .map_err(FastPayRpcV2DecodeError::Malformed)?,
                        );
                    }
                    2 => {
                        if votes.len() >= 16 {
                            return Err(FastPayRpcV2DecodeError::Malformed(
                                "too many votes in apply_full_req".to_string(),
                            ));
                        }
                        votes.push(
                            decode_vote(
                                wire_bytes(&v, "apply_full.vote")
                                    .map_err(FastPayRpcV2DecodeError::Malformed)?,
                            )
                            .map_err(FastPayRpcV2DecodeError::Malformed)?,
                        );
                    }
                    _ => {}
                }
            }
            let signed = signed_opt.ok_or_else(|| {
                FastPayRpcV2DecodeError::Malformed(
                    "apply_full_req missing signed_order".to_string(),
                )
            })?;
            FastPayRpcV2MethodPayload::OwnedApplyV3Full(OwnedTransferCertificateV3 {
                order: signed.order,
                owner_pubkey_hex: signed.owner_pubkey_hex,
                owner_signature_hex: signed.owner_signature_hex,
                votes,
            })
        }
        other => {
            return Err(FastPayRpcV2DecodeError::Malformed(format!(
                "unknown method {other}"
            )))
        }
    };

    Ok(FastPayRpcV2Request {
        request_id,
        validator_id,
        payload,
    })
}

pub fn encode_fastpay_rpc_v2_response_frame(
    resp: &FastPayRpcV2Response,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::with_capacity(4096);
    pb_varint(1, resp.request_id, &mut body);
    pb_varint(2, u64::from(resp.status), &mut body);
    pb_string(3, &resp.error_message, &mut body);
    if let Some(vote) = &resp.vote {
        pb_bytes(4, &encode_vote(vote)?, &mut body);
    }
    if let Some(outcome) = &resp.apply_outcome {
        let encoded = match outcome {
            FastPayApplyOutcomeV3::Ack(ack) => encode_apply_ack(ack)?,
            FastPayApplyOutcomeV3::Held {
                validator_id,
                lock_id,
                certificate_digest,
            } => encode_held_effect(validator_id, lock_id, certificate_digest)?,
        };
        pb_bytes(5, &encoded, &mut body);
    }
    let mut frame = Vec::with_capacity(5 + body.len());
    frame.push(FASTPAY_RPC_V2_VERSION);
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

// ---------------------------------------------------------------------------
// Per-IP Token Bucket Rate Limiter
// ---------------------------------------------------------------------------

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

struct PerIpRateLimiter {
    rate_per_sec: f64,
    burst: f64,
    buckets: Mutex<HashMap<IpAddr, TokenBucket>>,
}

impl PerIpRateLimiter {
    fn new(rate_per_sec: f64, burst: f64) -> Self {
        Self {
            rate_per_sec,
            burst,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn allow(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if buckets.len() > 4096 {
            buckets.retain(|_, b| now.duration_since(b.last_refill) < Duration::from_secs(60));
        }
        let bucket = buckets.entry(ip).or_insert(TokenBucket {
            tokens: self.burst,
            last_refill: now,
        });
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.rate_per_sec).min(self.burst);
        bucket.last_refill = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Direct TLS 1.3 TCP Listener
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FastPayRpcV2ServerConfig {
    pub data_dir: PathBuf,
    pub bind_host: String,
    pub tls_port: u16,
    pub tls_cert_path: PathBuf,
    pub tls_key_path: PathBuf,
    pub tls_client_ca_path: Option<PathBuf>,
    pub event_log: Option<PathBuf>,
    pub node_id: String,
    pub timeout_ms: u64,
}

pub fn build_fastpay_v2_tls_server_config(
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
    client_ca_path: Option<&std::path::Path>,
) -> Result<Arc<ServerConfig>, String> {
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert_path)
        .map_err(|e| format!("failed to open TLS cert `{}`: {e}", cert_path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("failed to parse TLS cert `{}`: {e}", cert_path.display()))?;
    if certs.is_empty() {
        return Err(format!("TLS cert file `{}` is empty", cert_path.display()));
    }
    let key = PrivateKeyDer::from_pem_file(key_path)
        .map_err(|e| format!("failed to parse TLS key `{}`: {e}", key_path.display()))?;

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| format!("TLS 1.3 server config failed: {e}"))?;

    let config = if let Some(ca_path) = client_ca_path {
        let mut roots = RootCertStore::empty();
        let ca_certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(ca_path)
            .map_err(|e| format!("failed to open client CA `{}`: {e}", ca_path.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("failed to parse client CA `{}`: {e}", ca_path.display()))?;
        for cert in ca_certs {
            roots
                .add(cert)
                .map_err(|e| format!("failed to add client CA cert: {e}"))?;
        }
        let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|e| format!("mTLS client verifier build failed: {e}"))?;
        builder
            .with_client_cert_verifier(verifier)
            .with_single_cert(certs, key)
            .map_err(|e| format!("mTLS server config failed: {e}"))?
    } else {
        builder
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| format!("TLS 1.3 server config failed: {e}"))?
    };

    Ok(Arc::new(config))
}

pub fn dispatch_fastpay_rpc_v2_request(
    data_dir: &std::path::Path,
    req: FastPayRpcV2Request,
) -> FastPayRpcV2Response {
    let options = NodeOptions {
        data_dir: data_dir.to_path_buf(),
    };
    match req.payload {
        FastPayRpcV2MethodPayload::OwnedSignV3(signed) => {
            match owned_sign_v3_typed(options, signed, &req.validator_id) {
                Ok(vote) => FastPayRpcV2Response {
                    request_id: req.request_id,
                    status: STATUS_OK,
                    error_message: String::new(),
                    vote: Some(vote),
                    apply_outcome: None,
                },
                Err(error) => FastPayRpcV2Response {
                    request_id: req.request_id,
                    status: STATUS_INVALID_INPUT,
                    error_message: error.to_string(),
                    vote: None,
                    apply_outcome: None,
                },
            }
        }
        FastPayRpcV2MethodPayload::OwnedApplyV3Fast { lock_id, votes } => {
            match owned_apply_v3_fast_typed(options, &lock_id, votes, &req.validator_id) {
                Ok(outcome) => FastPayRpcV2Response {
                    request_id: req.request_id,
                    status: STATUS_OK,
                    error_message: String::new(),
                    vote: None,
                    apply_outcome: Some(outcome),
                },
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        && error.to_string().contains("OrderNotCached") =>
                {
                    FastPayRpcV2Response {
                        request_id: req.request_id,
                        status: STATUS_ORDER_NOT_CACHED,
                        error_message: "OrderNotCached".to_string(),
                        vote: None,
                        apply_outcome: None,
                    }
                }
                Err(error) => FastPayRpcV2Response {
                    request_id: req.request_id,
                    status: STATUS_INVALID_INPUT,
                    error_message: error.to_string(),
                    vote: None,
                    apply_outcome: None,
                },
            }
        }
        FastPayRpcV2MethodPayload::OwnedApplyV3Full(cert) => {
            match owned_apply_v3_typed(options, cert, &req.validator_id) {
                Ok(outcome) => FastPayRpcV2Response {
                    request_id: req.request_id,
                    status: STATUS_OK,
                    error_message: String::new(),
                    vote: None,
                    apply_outcome: Some(outcome),
                },
                Err(error) => FastPayRpcV2Response {
                    request_id: req.request_id,
                    status: STATUS_INVALID_INPUT,
                    error_message: error.to_string(),
                    vote: None,
                    apply_outcome: None,
                },
            }
        }
    }
}

fn handle_framed_stream<S: Read + Write>(
    stream: &mut S,
    peer_ip: IpAddr,
    data_dir: &std::path::Path,
    node_id: &str,
    event_log: Option<&Mutex<std::fs::File>>,
    limiter: &PerIpRateLimiter,
) {
    let mut header = [0u8; 5];
    loop {
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(_) => break,
        }
        let started = Instant::now();
        let version = header[0];
        let frame_len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
        if version != FASTPAY_RPC_V2_VERSION {
            let err_resp = FastPayRpcV2Response {
                request_id: 0,
                status: STATUS_UNSUPPORTED_PROTOCOL_VERSION,
                error_message: format!(
                    "UnsupportedProtocolVersion: got {version}, expected {FASTPAY_RPC_V2_VERSION}"
                ),
                vote: None,
                apply_outcome: None,
            };
            if let Ok(bytes) = encode_fastpay_rpc_v2_response_frame(&err_resp) {
                let _ = stream.write_all(&bytes);
                let _ = stream.flush();
            }
            break;
        }
        if frame_len == 0 || frame_len > MAX_FASTPAY_V2_FRAME_BYTES {
            let err_resp = FastPayRpcV2Response {
                request_id: 0,
                status: STATUS_INVALID_INPUT,
                error_message: format!(
                    "frame length {frame_len} exceeds cap {MAX_FASTPAY_V2_FRAME_BYTES}"
                ),
                vote: None,
                apply_outcome: None,
            };
            if let Ok(bytes) = encode_fastpay_rpc_v2_response_frame(&err_resp) {
                let _ = stream.write_all(&bytes);
                let _ = stream.flush();
            }
            break;
        }
        let mut body = vec![0u8; frame_len];
        if stream.read_exact(&mut body).is_err() {
            break;
        }
        if !limiter.allow(peer_ip) {
            let err_resp = FastPayRpcV2Response {
                request_id: 0,
                status: STATUS_RATE_LIMITED,
                error_message: "rate limit exceeded".to_string(),
                vote: None,
                apply_outcome: None,
            };
            if let Ok(bytes) = encode_fastpay_rpc_v2_response_frame(&err_resp) {
                let _ = stream.write_all(&bytes);
                let _ = stream.flush();
            }
            continue;
        }

        let (method_label, response) = match decode_fastpay_rpc_v2_body(&body) {
            Ok(req) => {
                let label = match &req.payload {
                    FastPayRpcV2MethodPayload::OwnedSignV3(_) => "owned_sign_v3_v2",
                    FastPayRpcV2MethodPayload::OwnedApplyV3Fast { .. } => "owned_apply_v3_fast_v2",
                    FastPayRpcV2MethodPayload::OwnedApplyV3Full(_) => "owned_apply_v3_full_v2",
                };
                (label, dispatch_fastpay_rpc_v2_request(data_dir, req))
            }
            Err(FastPayRpcV2DecodeError::UnsupportedProtocolVersion(v)) => (
                "invalid_v2",
                FastPayRpcV2Response {
                    request_id: 0,
                    status: STATUS_UNSUPPORTED_PROTOCOL_VERSION,
                    error_message: format!("UnsupportedProtocolVersion: {v}"),
                    vote: None,
                    apply_outcome: None,
                },
            ),
            Err(error) => (
                "invalid_v2",
                FastPayRpcV2Response {
                    request_id: 0,
                    status: STATUS_INVALID_INPUT,
                    error_message: error.to_string(),
                    vote: None,
                    apply_outcome: None,
                },
            ),
        };
        let fp_metrics = take_fastpay_rpc_metrics();
        let handler_ms = started.elapsed().as_secs_f64() * 1000.0;
        if let Some(log_lock) = event_log {
            if let Ok(mut file) = log_lock.lock() {
                let record = serde_json::json!({
                    "schema": "postfiat-rpc-serve-event-v1",
                    "node_id": node_id,
                    "peer_addr": peer_ip.to_string(),
                    "id": format!("v2-{}", response.request_id),
                    "method": method_label,
                    "ok": response.status == STATUS_OK,
                    "handler_mode": "fastpay_rpc_v2_tls13",
                    "handler_ms": (handler_ms * 1000.0).round() / 1000.0,
                    "fastpay_redb_lease_wait_ms": fp_metrics.as_ref().and_then(|m| m.redb_lease_wait_ms),
                    "fastpay_wal_fdatasync_ms": fp_metrics.as_ref().and_then(|m| m.wal_fdatasync_ms),
                    "fastpay_ledger_cache_hit": fp_metrics.as_ref().and_then(|m| m.ledger_cache_hit),
                    "fastpay_redb_opened": fp_metrics.as_ref().and_then(|m| m.redb_opened),
                    "fastpay_order_lru_hit": fp_metrics.as_ref().and_then(|m| m.order_lru_hit),
                    "error_code": if response.status == STATUS_OK { serde_json::Value::Null } else { serde_json::Value::String(response.error_message.clone()) },
                });
                if let Ok(line) = serde_json::to_string(&record) {
                    let _ = writeln!(file, "{line}");
                }
            }
        }
        let Ok(encoded) = encode_fastpay_rpc_v2_response_frame(&response) else {
            break;
        };
        if stream.write_all(&encoded).is_err() || stream.flush().is_err() {
            break;
        }
    }
}

pub struct FastPayRpcV2ListenerHandle {
    stop: Arc<AtomicBool>,
    pub bound_addr: std::net::SocketAddr,
}

impl Drop for FastPayRpcV2ListenerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn spawn_fastpay_rpc_v2_tls_listener(
    cfg: FastPayRpcV2ServerConfig,
) -> Result<FastPayRpcV2ListenerHandle, String> {
    let tls_config = build_fastpay_v2_tls_server_config(
        &cfg.tls_cert_path,
        &cfg.tls_key_path,
        cfg.tls_client_ca_path.as_deref(),
    )?;
    let bind_addr = format!("{}:{}", cfg.bind_host, cfg.tls_port);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|e| format!("fastpay-v2 TLS bind `{bind_addr}` failed: {e}"))?;
    let bound_addr = listener
        .local_addr()
        .map_err(|e| format!("fastpay-v2 local_addr failed: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("fastpay-v2 set_nonblocking failed: {e}"))?;

    let event_log = cfg
        .event_log
        .as_ref()
        .and_then(|path| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
        })
        .map(|f| Arc::new(Mutex::new(f)));

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let stop_prewarmer = Arc::clone(&stop);
    let prewarmer_data_dir = cfg.data_dir.clone();
    let _ = std::thread::Builder::new()
        .name("fastpay-cache-prewarmer".to_string())
        .spawn(move || {
            let _ = prewarm_fastpay_ledger_cache_from_sidecar(&prewarmer_data_dir);
            while !stop_prewarmer.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(25));
                let _ = prewarm_fastpay_ledger_cache_from_sidecar(&prewarmer_data_dir);
            }
        });
    let active_conns = Arc::new(AtomicUsize::new(0));
    let limiter = Arc::new(PerIpRateLimiter::new(
        DEFAULT_FASTPAY_V2_RATE_LIMIT_PER_SEC,
        DEFAULT_FASTPAY_V2_RATE_BURST,
    ));

    std::thread::Builder::new()
        .name("fastpay-rpc-v2-tls".to_string())
        .spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((tcp_stream, peer_addr)) => {
                        if active_conns.load(Ordering::Relaxed)
                            >= MAX_FASTPAY_V2_CONCURRENT_CONNECTIONS
                        {
                            drop(tcp_stream);
                            continue;
                        }
                        let _ = tcp_stream.set_nodelay(true);
                        let timeout = Some(Duration::from_millis(cfg.timeout_ms.max(1000)));
                        let _ = tcp_stream.set_read_timeout(timeout);
                        let _ = tcp_stream.set_write_timeout(timeout);

                        let tls_cfg = Arc::clone(&tls_config);
                        let data_dir = cfg.data_dir.clone();
                        let node_id = cfg.node_id.clone();
                        let event_log = event_log.clone();
                        let limiter = Arc::clone(&limiter);
                        let active = Arc::clone(&active_conns);
                        active.fetch_add(1, Ordering::Relaxed);
                        let _ = std::thread::spawn(move || {
                            struct ConnGuard(Arc<AtomicUsize>);
                            impl Drop for ConnGuard {
                                fn drop(&mut self) {
                                    self.0.fetch_sub(1, Ordering::Relaxed);
                                }
                            }
                            let _guard = ConnGuard(active);
                            if let Ok(conn) = ServerConnection::new(tls_cfg) {
                                let mut tls_stream = StreamOwned::new(conn, tcp_stream);
                                handle_framed_stream(
                                    &mut tls_stream,
                                    peer_addr.ip(),
                                    &data_dir,
                                    &node_id,
                                    event_log.as_deref(),
                                    &limiter,
                                );
                            }
                        });
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| format!("failed to spawn fastpay-rpc-v2 thread: {e}"))?;

    Ok(FastPayRpcV2ListenerHandle { stop, bound_addr })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fastpay_v2_decoder_rejects_bad_version_oversized_and_fuzz_inputs() {
        assert_eq!(
            decode_fastpay_rpc_v2_frame(&[1, 0, 0, 0, 1, 0]),
            Err(FastPayRpcV2DecodeError::UnsupportedProtocolVersion(1))
        );
        assert_eq!(
            decode_fastpay_rpc_v2_frame(&[2, 0x00, 0x01, 0x00, 0x01]),
            Err(FastPayRpcV2DecodeError::FrameTooLarge(65_537))
        );
        // Deterministic pseudo-random fuzz corpus: decoder must NEVER panic on arbitrary bytes.
        let mut seed: u64 = 0x5046_5632_CAFE_BABE;
        for case in 0..512 {
            let len = (case % 96) + 1;
            let mut buf = Vec::with_capacity(5 + len);
            buf.push(FASTPAY_RPC_V2_VERSION);
            buf.extend_from_slice(&(len as u32).to_be_bytes());
            for _ in 0..len {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                buf.push((seed >> 33) as u8);
            }
            let _ = decode_fastpay_rpc_v2_frame(&buf);
        }
    }
}
