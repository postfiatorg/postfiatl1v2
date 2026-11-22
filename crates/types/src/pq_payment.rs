//! Compact pq_payment_v1. Integers are little endian. Asset zero is PFT;
//! a nonzero 48-byte selector is the exact governed pfUSDC source-series ID.
use base64::{engine::general_purpose::STANDARD, Engine};
use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes, validate_canonical_address};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha384};

pub const ARCHIVE_LIMIT: usize = 149_130;
pub const REQUEST_LIMIT: usize = 1_048_576;
pub const INNER_LIMIT: usize = 140_476;
pub const INNER_PREFIX: usize = 3_648;
pub const PUBLIC_KEY_BYTES: usize = 1952;
pub const SIGNATURE_BYTES: usize = 3309;
const PREFIX: usize = 4 + 1 + 1 + 1 + 48 + 8 + 8 + 1;
const ACCOUNT_FIELDS: usize = 20 + 8 + PUBLIC_KEY_BYTES;
pub const POOL_ID: &str = "PFT-PQ-SHIELDED-POOL-V1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Mode {
    Internal = 0,
    Shield = 1,
    Exit = 2,
}

pub fn destination(address: &str) -> Result<[u8; 32], &'static str> {
    validate_canonical_address(address).map_err(|_| "noncanonical PQ destination")?;
    let mut out = [0; 32];
    out[12..].copy_from_slice(&hex_to_bytes(&address[2..]).map_err(|_| "bad account id")?);
    if out == [0; 32] {
        return Err("zero PQ destination");
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PqPayment(Vec<u8>);

#[derive(Clone, Copy)]
pub struct Chain<'a> {
    pub id: &'a str,
    pub genesis: [u8; 48],
    pub protocol: u32,
}

#[derive(Debug, Clone)]
pub struct Terms {
    pub mode: Mode,
    pub asset: [u8; 48],
    pub epoch: u64,
    pub public_fee: u64,
    pub account: Option<Account>,
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: [u8; 20],
    pub sequence: u64,
    pub public_key: Box<[u8; PUBLIC_KEY_BYTES]>,
}

impl Terms {
    fn prefix(&self) -> Vec<u8> {
        let mut out = b"PFPQ".to_vec();
        out.extend([1, 1, self.mode as u8]); // outer version, L1 profile, public edge
        out.extend(self.asset);
        out.extend(self.epoch.to_le_bytes());
        out.extend(self.public_fee.to_le_bytes());
        out.push(self.account.is_some() as u8);
        if let Some(a) = &self.account {
            out.extend(a.id);
            out.extend(a.sequence.to_le_bytes());
            out.extend_from_slice(a.public_key.as_ref());
        }
        out
    }

    pub fn statement_context(&self, chain: Chain, raw: [u8; 48]) -> [u8; 48] {
        let mut h = Sha384::new();
        h.update(b"PFT-SH-L1-CONTEXT-V1");
        h.update((chain.id.len() as u64).to_le_bytes());
        h.update(chain.id.as_bytes());
        h.update(chain.genesis);
        h.update(chain.protocol.to_le_bytes());
        h.update(POOL_ID.as_bytes());
        h.update(self.prefix());
        h.update(raw);
        h.finalize().into()
    }

    pub fn signing_message(&self, chain: Chain, raw: [u8; 48]) -> Option<Vec<u8>> {
        let account = self.account.as_ref()?;
        let mut out = if self.mode == Mode::Shield {
            b"PFT-SH-MINT-AUTH-V2".to_vec()
        } else {
            b"PFT-SH-FEE-AUTH-V1".to_vec()
        };
        out.extend(account.id);
        out.extend(raw);
        out.extend(48u32.to_le_bytes());
        out.extend(self.statement_context(chain, raw));
        Some(out)
    }
}

impl PqPayment {
    pub fn encode(
        terms: &Terms,
        signature: Option<&[u8; SIGNATURE_BYTES]>,
        inner: &[u8],
    ) -> Result<Self, &'static str> {
        if terms.account.is_some() != signature.is_some() || inner.len() > INNER_LIMIT {
            return Err("invalid PQ authorization or length");
        }
        let mut bytes = terms.prefix();
        if let Some(sig) = signature {
            bytes.extend(sig);
        }
        bytes.extend(inner);
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, &'static str> {
        let p = Self(bytes);
        p.parts()?;
        Ok(p)
    }

    pub fn digest(&self) -> String {
        postfiat_crypto_provider::hash_hex("postfiat.pq_payment.body.v1", &self.0)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn parts(&self) -> Result<(Terms, Option<&[u8; SIGNATURE_BYTES]>, &[u8]), &'static str> {
        let b = &self.0;
        if b.len() > ARCHIVE_LIMIT || b.len() < PREFIX || &b[..6] != b"PFPQ\x01\x01" {
            return Err("unknown PQ encoding or profile");
        }
        let mode = match b[6] {
            0 => Mode::Internal,
            1 => Mode::Shield,
            2 => Mode::Exit,
            _ => return Err("unknown PQ mode"),
        };
        let asset = b[7..55].try_into().unwrap();
        let epoch = u64::from_le_bytes(b[55..63].try_into().unwrap());
        let public_fee = u64::from_le_bytes(b[63..71].try_into().unwrap());
        let issued = asset != [0; 48];
        let needs_auth = mode == Mode::Shield || issued;
        if b[71] != needs_auth as u8 || (public_fee != 0) != issued {
            return Err("wrong PQ public fee or authorization");
        }
        let mut rest = &b[PREFIX..];
        let (account, signature) = if needs_auth {
            if rest.len() < ACCOUNT_FIELDS + SIGNATURE_BYTES {
                return Err("truncated PQ authorization");
            }
            let id = rest[..20].try_into().unwrap();
            let sequence = u64::from_le_bytes(rest[20..28].try_into().unwrap());
            if id == [0; 20] || sequence == 0 {
                return Err("invalid PQ authorizing account");
            }
            let public_key = Box::new(rest[28..ACCOUNT_FIELDS].try_into().unwrap());
            let signature = rest[ACCOUNT_FIELDS..ACCOUNT_FIELDS + SIGNATURE_BYTES]
                .try_into()
                .unwrap();
            rest = &rest[ACCOUNT_FIELDS + SIGNATURE_BYTES..];
            (
                Some(Account {
                    id,
                    sequence,
                    public_key,
                }),
                Some(signature),
            )
        } else {
            (None, None)
        };
        if !(INNER_PREFIX..=INNER_LIMIT).contains(&rest.len()) || &rest[..8] != b"PFSH02\0\0" {
            return Err("invalid PQ inner frame");
        }
        let mint = u64::from_le_bytes(rest[56..64].try_into().unwrap());
        let exit = u64::from_le_bytes(rest[64..72].try_into().unwrap());
        let fee = u64::from_le_bytes(rest[104..112].try_into().unwrap());
        if (mint > 0) != (mode == Mode::Shield)
            || (exit > 0) != (mode == Mode::Exit)
            || exit.checked_add(fee).is_none()
            || (issued && fee != 0)
        {
            return Err("inconsistent PQ public flow");
        }
        if exit > 0 {
            let address = format!("pf{}", bytes_to_hex(&rest[84..104]));
            if rest[72..104] != destination(&address)? {
                return Err("noncanonical PQ exit");
            }
        } else if rest[72..104] != [0; 32] {
            return Err("unexpected PQ destination");
        }
        let proof_len =
            u32::from_le_bytes(rest[INNER_PREFIX - 4..INNER_PREFIX].try_into().unwrap()) as usize;
        if proof_len != rest.len() - INNER_PREFIX || proof_len == 0 {
            return Err("invalid PQ proof length");
        }
        Ok((
            Terms {
                mode,
                asset,
                epoch,
                public_fee,
                account,
            },
            signature,
            rest,
        ))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transport<'a> {
    #[serde(borrow)]
    body: std::borrow::Cow<'a, str>,
}

impl Serialize for PqPayment {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Transport {
            body: STANDARD.encode(&self.0).into(),
        }
        .serialize(s)
    }
}
impl<'de> Deserialize<'de> for PqPayment {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let transport = Transport::deserialize(d)?;
        if transport.body.len() > ARCHIVE_LIMIT.div_ceil(3) * 4 {
            return Err(D::Error::custom("oversize PQ body"));
        }
        let bytes = STANDARD
            .decode(transport.body.as_bytes())
            .map_err(D::Error::custom)?;
        Self::from_bytes(bytes).map_err(D::Error::custom)
    }
}

/// Archive budgets apply to the serialized payment and its share of framing.
pub fn checked_archive_size(body: usize, framing: usize) -> Result<usize, &'static str> {
    body.checked_add(framing)
        .filter(|n| *n <= ARCHIVE_LIMIT)
        .ok_or("PQ archive limit exceeded")
}

// Consensus metadata is small even for a full depth-24 epoch. The historical
// indexes are derived from immutable pair pages and never deserialized from JSON.
#[path = "pq_tree.rs"]
mod tree;
pub use tree::{PqEpoch, PqIndexes, PQ_CAPACITY, PQ_DEPTH, PQ_WINDOW};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PqAsset {
    pub epoch: u64,
    pub reserve: u64,
    pub ingress: u128,
    pub exits: u128,
    pub fees: u128,
    pub frozen_epochs_root: String,
    pub writable: PqEpoch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PqOutput {
    pub commitment: String,
    // Canonical padded base64, exactly one 1,670-byte PFSH02 envelope.
    pub ciphertext: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PqPair {
    pub previous: String,
    pub asset: String,
    pub read_epoch: u64,
    pub anchor: String,
    pub epoch: u64,
    pub first_index: u32,
    pub height: u64,
    pub kind: Mode,
    pub mint: u64,
    pub exit: u64,
    pub fee: u64,
    pub serials: [String; 2],
    pub outputs: [PqOutput; 2],
}

impl PqPair {
    pub fn digest(&self) -> String {
        digest_record("postfiat.pq.pair.v1", self)
    }
}

pub(super) fn digest_record(tag: &str, value: &impl Serialize) -> String {
    // Typed tuple/struct fields have fixed order. JSON integers are canonical
    // decimal and every string in the data model has a single checked encoding.
    postfiat_crypto_provider::hash_hex(tag, &serde_json::to_vec(value).expect("typed PQ record"))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PqHead {
    pub pairs: u64,
    pub last_page: String,
    pub serial_index_root: String,
    pub commitment_index_root: String,
    pub output_index_root: String,
    pub assets: std::collections::BTreeMap<String, PqAsset>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PqPool {
    pub head: PqHead,
    #[serde(skip)]
    pub indexes: PqIndexes,
    #[serde(skip)]
    pub pending: imbl::Vector<std::sync::Arc<PqPair>>,
}

impl PartialEq for PqPool {
    fn eq(&self, rhs: &Self) -> bool {
        self.head == rhs.head
    }
}
impl Eq for PqPool {}
