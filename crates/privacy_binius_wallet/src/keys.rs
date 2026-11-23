use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use pft_binius_shielded::payment::{ShieldedWallet, StealthAddress};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha384};

const SALT: &[u8] = b"postfiat/pq-wallet/v1/hkdf-salt";
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Network {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

impl Network {
    pub fn genesis(&self) -> Result<[u8; 48], &'static str> {
        if self.chain_id.is_empty() || self.chain_id.len() > 255 {
            return Err("invalid network");
        }
        hex::decode(&self.genesis_hash)
            .ok()
            .and_then(|s| s.try_into().ok())
            .ok_or("invalid genesis")
    }

    fn frame(&self) -> Result<Vec<u8>, &'static str> {
        let mut buf = Vec::new();
        field(&mut buf, self.chain_id.as_bytes());
        field(&mut buf, &self.genesis()?);
        Ok(buf)
    }
}
fn field(to: &mut Vec<u8>, bytes: &[u8]) {
    to.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    to.extend_from_slice(bytes);
}
fn mac(key: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    let mut h = Hmac::<Sha384>::new_from_slice(key).expect("HMAC key");
    for part in parts {
        h.update(part);
    }
    h.finalize().into_bytes().to_vec()
}
pub fn derive(
    seed: &[u8; 48],
    network: &Network,
    account: u32,
    index: u32,
) -> Result<ShieldedWallet, &'static str> {
    let network = network.frame()?;
    let prk = mac(SALT, &[seed]);
    let expand = |label: &[u8], length: usize| {
        let mut info = Vec::new();
        field(&mut info, label);
        info.extend_from_slice(&network);
        field(&mut info, &account.to_be_bytes());
        field(&mut info, &index.to_be_bytes());
        let first = mac(&prk, &[&info, &[1]]);
        let mut result = first.clone();
        if length > first.len() {
            result.extend(mac(&prk, &[&first, &info, &[2]]));
        }
        result.truncate(length);
        result
    };
    Ok(ShieldedWallet::from_secrets(
        expand(b"postfiat/pq-wallet/v1/spend", 48)
            .try_into()
            .unwrap(),
        expand(b"postfiat/pq-wallet/v1/kem", 64).try_into().unwrap(),
    ))
}
pub fn address(network: &Network, dest: &StealthAddress) -> Result<String, &'static str> {
    let mut raw = vec![1];
    raw.extend(Sha384::digest(network.frame()?));
    raw.extend(dest.addr);
    raw.extend(dest.ek_bytes);
    raw.extend_from_slice(&Sha384::digest(&raw)[..12]);
    Ok(format!("pfpq1{}", URL_SAFE_NO_PAD.encode(raw)))
}
pub fn decode(network: &Network, text: &str) -> Result<StealthAddress, &'static str> {
    let raw = text
        .strip_prefix("pfpq1")
        .and_then(|v| URL_SAFE_NO_PAD.decode(v).ok())
        .ok_or("invalid receive address")?;
    if raw.len() != 1677
        || raw[0] != 1
        || raw[1..49] != Sha384::digest(network.frame()?)[..]
        || raw[1665..] != Sha384::digest(&raw[..1665])[..12]
    {
        return Err("address checksum or network mismatch");
    }
    Ok(StealthAddress {
        addr: raw[49..97].try_into().unwrap(),
        ek_bytes: raw[97..1665].try_into().unwrap(),
    })
}
