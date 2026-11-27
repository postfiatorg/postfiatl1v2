//! Public PFSH02 receive-address encoding, shared by the native wallet and WASM.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha384};

pub fn network_frame(chain: &str, genesis: &[u8; 48]) -> Result<Vec<u8>, &'static str> {
    if chain.is_empty() || chain.len() > 255 {
        return Err("invalid network");
    }
    let mut frame = Vec::new();
    for field in [chain.as_bytes(), genesis] {
        frame.extend((field.len() as u32).to_be_bytes());
        frame.extend(field);
    }
    Ok(frame)
}

pub fn encode(network: &[u8], owner: &[u8; 48], kem: &[u8; 1568]) -> String {
    let mut raw = vec![1];
    raw.extend(Sha384::digest(network));
    raw.extend(owner);
    raw.extend(kem);
    raw.extend_from_slice(&Sha384::digest(&raw)[..12]);
    format!("pfpq1{}", URL_SAFE_NO_PAD.encode(raw))
}

pub fn decode(network: &[u8], text: &str) -> Result<([u8; 48], [u8; 1568]), &'static str> {
    if text.len() != 2241 {
        return Err("invalid receive address");
    }
    let raw = text
        .strip_prefix("pfpq1")
        .and_then(|s| URL_SAFE_NO_PAD.decode(s).ok())
        .ok_or("invalid receive address")?;
    if raw.len() != 1677
        || raw[0] != 1
        || raw[1..49] != Sha384::digest(network)[..]
        || raw[1665..] != Sha384::digest(&raw[..1665])[..12]
    {
        return Err("address checksum or network mismatch");
    }
    Ok((
        raw[49..97].try_into().unwrap(),
        raw[97..1665].try_into().unwrap(),
    ))
}
