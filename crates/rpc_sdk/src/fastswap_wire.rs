use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use std::io::{Read, Write};

pub const FASTSWAP_WIRE_GZIP_BASE64_V2: &str = "gzip-base64-v2";
pub const FASTSWAP_WIRE_GZIP_BASE64_V2_PREFIX: &str = "postfiat-fastswap-wire-gzip-base64-v2:";
const MAX_FASTSWAP_WIRE_DECODED_BYTES: usize = 8 * 1024 * 1024;
const MAX_FASTSWAP_WIRE_COMPRESSION_RATIO: usize = 64;

/// Encode only the transport body. Canonical intent, vote, and certificate
/// bytes remain unchanged and continue to be signature-verified after decode.
pub fn encode_fastswap_wire_payload_v2(json: &str) -> Result<String, String> {
    if json.len() > MAX_FASTSWAP_WIRE_DECODED_BYTES {
        return Err("FastSwap wire payload exceeds decoded byte limit".to_owned());
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(json.as_bytes())
        .map_err(|error| format!("FastSwap wire compression failed: {error}"))?;
    let compressed = encoder
        .finish()
        .map_err(|error| format!("FastSwap wire compression failed: {error}"))?;
    Ok(format!(
        "{FASTSWAP_WIRE_GZIP_BASE64_V2_PREFIX}{}",
        STANDARD_NO_PAD.encode(compressed)
    ))
}

pub fn decode_fastswap_wire_payload_v2(value: &str) -> Result<Vec<u8>, String> {
    let encoded = value
        .strip_prefix(FASTSWAP_WIRE_GZIP_BASE64_V2_PREFIX)
        .ok_or_else(|| "FastSwap wire payload has an unsupported codec".to_owned())?;
    let compressed = STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|error| format!("FastSwap wire base64 decode failed: {error}"))?;
    if compressed.is_empty() {
        return Err("FastSwap wire compressed payload is empty".to_owned());
    }
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut decoded = Vec::new();
    decoder
        .by_ref()
        .take((MAX_FASTSWAP_WIRE_DECODED_BYTES + 1) as u64)
        .read_to_end(&mut decoded)
        .map_err(|error| format!("FastSwap wire decompression failed: {error}"))?;
    if decoded.len() > MAX_FASTSWAP_WIRE_DECODED_BYTES
        || decoded.len()
            > compressed
                .len()
                .saturating_mul(MAX_FASTSWAP_WIRE_COMPRESSION_RATIO)
    {
        return Err("FastSwap wire decompression bounds exceeded".to_owned());
    }
    Ok(decoded)
}

/// Decode a negotiated wire-v2 response result back into the legacy typed
/// JSON value consumed by the existing verification path.
pub fn decode_fastswap_wire_result_v2(value: &str) -> Result<serde_json::Value, String> {
    let decoded = decode_fastswap_wire_payload_v2(value)?;
    serde_json::from_slice(&decoded)
        .map_err(|error| format!("FastSwap wire response JSON decode failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_wire_round_trips_and_rejects_unknown_codec() {
        let mut state = 0x1234_5678u32;
        let payload = (0..64_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                char::from(b'a' + (state % 26) as u8)
            })
            .collect::<String>();
        let json = format!("{{\"payload\":\"{payload}\"}}");
        let encoded = encode_fastswap_wire_payload_v2(&json).expect("encode");
        assert!(encoded.len() < json.len());
        assert_eq!(
            decode_fastswap_wire_payload_v2(&encoded).expect("decode"),
            json.as_bytes()
        );
        assert!(decode_fastswap_wire_payload_v2("plain-json").is_err());
    }

    #[test]
    fn compressed_wire_result_round_trips_typed_json() {
        let mut state = 0x9e37_79b9u32;
        let signature = (0..4_627)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect::<Vec<_>>();
        let value = serde_json::json!({
            "validator_id": "validator-4",
            "signature": signature,
            "accepted": true,
        });
        let json = serde_json::to_string(&value).expect("JSON");
        let encoded = encode_fastswap_wire_payload_v2(&json).expect("encode");
        assert_eq!(
            decode_fastswap_wire_result_v2(&encoded).expect("decode"),
            value
        );
    }
}
