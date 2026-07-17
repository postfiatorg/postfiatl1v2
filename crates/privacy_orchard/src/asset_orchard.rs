use ff::{Field, PrimeField, PrimeFieldBits};
use group::{prime::PrimeCurveAffine, Curve, Group, GroupEncoding};
use halo2_poseidon::{Mds, P128Pow5T3, Spec, State};
use orchard::primitives::redpallas::{SigningKey, SpendAuth, VerificationKey};
use pasta_curves::{
    arithmetic::{Coordinates, CurveAffine, CurveExt},
    pallas,
};
use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha3::{digest::Update, Digest, Sha3_256, Sha3_384, Sha3_512};
use std::ops::Deref;
use zeroize::Zeroize;

use crate::asset_orchard_sinsemilla::asset_spend_auth_g;

pub const ASSET_ORCHARD_ACTION_VERSION_V1: u16 = 1;
pub const ASSET_ORCHARD_LEG_COUNT: usize = 2;
pub const ASSET_ORCHARD_POOL_ID_V1: &str = "asset-orchard-v1";
pub const ASSET_ORCHARD_NOTE_VERSION_V1: u16 = 1;
pub const ASSET_ORCHARD_PROOF_SYSTEM_ID_V1: &str = "postfiat.privacy.asset-orchard-halo2.v1";
/// Replay-only identity for the pre-Pow5 swap verifying key.
pub const ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY: &str = "asset_orchard.swap.pricing_bound.v3";
/// Active Pow5 swap verifying-key identity.
pub const ASSET_ORCHARD_CIRCUIT_ID_V4: &str = "asset_orchard.swap.pricing_bound.v4";
/// Active swap circuit identity retained under the original Rust symbol for API stability.
pub const ASSET_ORCHARD_CIRCUIT_ID_V1: &str = ASSET_ORCHARD_CIRCUIT_ID_V4;
/// Replay-only identity for the pre-Pow5 private-egress verifying key.
pub const ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY: &str =
    "asset_orchard.private_egress.v1";
/// Active Pow5 private-egress verifying-key identity.
pub const ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2: &str = "asset_orchard.private_egress.v2";
/// Active private-egress identity retained under the original Rust symbol for API stability.
pub const ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1: &str =
    ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2;

// The circuit identity is consensus-recorded verifier dispatch metadata. The existing
// H_action constants stay frozen so the Pow5 proving/VK material is byte-identical to
// 3218ec53; the recorded identity is independently bound by each spend signature.
const ASSET_ORCHARD_SWAP_PROOF_BINDING_ID: &str = ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY;
const ASSET_ORCHARD_PRIVATE_EGRESS_PROOF_BINDING_ID: &str =
    ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY;

pub(crate) fn supported_asset_orchard_swap_circuit_id(
    circuit_id: &str,
) -> Result<&'static str, AssetOrchardError> {
    match circuit_id {
        ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY => Ok(ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY),
        ASSET_ORCHARD_CIRCUIT_ID_V4 => Ok(ASSET_ORCHARD_CIRCUIT_ID_V4),
        _ => Err(AssetOrchardError::new(
            "unsupported_asset_orchard_circuit",
            format!("unsupported asset-orchard circuit `{circuit_id}`"),
        )),
    }
}

pub(crate) fn supported_asset_orchard_private_egress_circuit_id(
    circuit_id: &str,
) -> Result<&'static str, AssetOrchardError> {
    match circuit_id {
        ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY => {
            Ok(ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY)
        }
        ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2 => {
            Ok(ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2)
        }
        _ => Err(AssetOrchardError::new(
            "unsupported_asset_orchard_private_egress_circuit",
            format!("unsupported asset-orchard private egress circuit `{circuit_id}`"),
        )),
    }
}
pub const ASSET_ORCHARD_ACTION_SCHEMA_V1: &str = "postfiat-asset-orchard-swap-action-v2";
pub const ASSET_ORCHARD_DISCLOSED_EGRESS_SCHEMA_V1: &str =
    "postfiat-asset-orchard-disclosed-egress-v1";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1: &str =
    "postfiat-asset-orchard-private-egress-action-v1";
pub const ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1: &str = "postfiat.asset_orchard.note_commit.v1";
pub const ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES: usize = 64;
pub const ASSET_ORCHARD_SIGHASH_BYTES: usize = 32;
pub const ASSET_ORCHARD_POINT_BYTES: usize = 32;
pub const ASSET_ORCHARD_FIELD_BYTES: usize = 32;
pub const ASSET_ORCHARD_RSEED_BYTES: usize = 32;
pub const ASSET_ORCHARD_DIVERSIFIER_BYTES: usize = 11;
pub const ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES: usize = 64;
pub const ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES: usize = 4096;
pub const ASSET_ORCHARD_PROOF_MAX_BYTES: usize = 1_048_576;
pub const ASSET_ORCHARD_MAX_ASSET_ID_BYTES: usize = 128;
pub const ASSET_ORCHARD_MAX_POOL_ID_BYTES: usize = 128;
pub const ASSET_ORCHARD_POSEIDON_WIDTH: usize = 3;
pub const ASSET_ORCHARD_POSEIDON_RATE: usize = 2;
pub const ASSET_ORCHARD_PUBLIC_INSTANCE_LEN: usize = 28;
pub const ASSET_ORCHARD_H_ACTION_FIELD_COUNT: usize = 32;
pub const ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT: usize = 34;
pub const ASSET_ORCHARD_NOTE_MESSAGE_BITS: usize = 1597;
pub const ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS: usize = 1600;
pub const ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS: usize = 250;
pub const ASSET_ORCHARD_NOTE_MESSAGE_PIECE_COUNT: usize =
    (ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS + ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS - 1)
        / ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS;

const HASH_TO_PALLAS_BASE_DOMAIN: &[u8] = b"postfiat.hash_to_pallas_base.v1";
const HASH_TO_PALLAS_SCALAR_DOMAIN: &[u8] = b"postfiat.hash_to_pallas_scalar.v1";
const CONST_FIELD_DST: &str = "postfiat.asset_orchard.const.v1";
const ASSET_TAG_DOMAIN: &[u8] = b"postfiat.asset_orchard.asset_tag.v1";
const POOL_DOMAIN_DST: &str = "postfiat.asset_orchard.pool_domain.v1";
const ORCHARD_PSI_DST: &str = "postfiat.asset_orchard.rseed.psi.v1";
const ORCHARD_RCM_DST: &str = "postfiat.asset_orchard.rseed.rcm.v1";
const NULLIFIER_HASH_NAME: &str = "postfiat.asset_orchard.nullifier.v1";
const OUTPUT_RHO_HASH_NAME: &str = "postfiat.asset_orchard.output_rho.v1";
const ENCRYPTED_OUTPUT_HASH_DOMAIN: &[u8] = b"postfiat.asset_orchard.encrypted_output_hash.v1";
const H_ACTION_HASH_NAME: &str = "postfiat.asset_orchard.h_action.v1";
const PRIVATE_EGRESS_H_ACTION_HASH_NAME: &str = "postfiat.asset_orchard.private_egress.h_action.v1";
const H_SIG_DOMAIN: &[u8] = b"postfiat.asset_orchard.swap.sighash.v1";
const EGRESS_H_SIG_DOMAIN: &[u8] = b"postfiat.asset_orchard.disclosed_egress.sighash.v1";
const PRIVATE_EGRESS_H_SIG_DOMAIN: &[u8] = b"postfiat.asset_orchard.private_egress.sighash.v1";
const PRIVATE_EGRESS_EXIT_BINDING_DOMAIN: &[u8] =
    b"postfiat.asset_orchard.private_egress.exit_binding.v1";
const ACCOUNTING_VALUE_COMMITMENT_G_DST: &str =
    "postfiat.asset_orchard.swap_accounting.value_commitment_g.v1";
const ACCOUNTING_VALUE_COMMITMENT_H_DST: &str =
    "postfiat.asset_orchard.swap_accounting.value_commitment_h.v1";
const ACCOUNTING_BLINDING_DST: &[u8] = b"postfiat.asset_orchard.swap_accounting.blinding.v1";
const NOTE_VERSION_CONST_NAME: &str = "asset_orchard_note_version_1";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN: usize = 13;
pub const ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_FIELD_COUNT: usize = 16;
pub const ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT: usize = 18;

mod u128_hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{value:032x}"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom(
                "expected a 32-character hexadecimal u128 string",
            ));
        }
        u128::from_str_radix(&value, 16).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardError {
    code: &'static str,
    message: String,
}

impl AssetOrchardError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for AssetOrchardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AssetOrchardError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetOrchardSecret<T: Zeroize>(T);

impl<T: Zeroize> AssetOrchardSecret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose_secret(&self) -> &T {
        &self.0
    }
}

impl<T: Zeroize> Deref for AssetOrchardSecret<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.expose_secret()
    }
}

impl<T: Zeroize> Drop for AssetOrchardSecret<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> std::fmt::Debug for AssetOrchardSecret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AssetOrchardSecret([redacted])")
    }
}

impl<T: Zeroize> From<T> for AssetOrchardSecret<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetOrchardFieldElement(String);

impl AssetOrchardFieldElement {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, AssetOrchardError> {
        let value = parse_fixed_lower_hex(
            "asset_orchard_field",
            value.into(),
            ASSET_ORCHARD_FIELD_BYTES,
        )?;
        parse_pallas_base(&value)?;
        Ok(Self(value))
    }

    pub fn from_field(field: pallas::Base) -> Self {
        Self(bytes_to_hex(&field_enc(field)))
    }

    pub fn to_field(&self) -> Result<pallas::Base, AssetOrchardError> {
        parse_pallas_base(&self.0)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AssetOrchardFieldElement {
    type Error = AssetOrchardError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<AssetOrchardFieldElement> for String {
    fn from(value: AssetOrchardFieldElement) -> Self {
        value.0
    }
}

impl Zeroize for AssetOrchardFieldElement {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetOrchardPoint(String);

impl AssetOrchardPoint {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, AssetOrchardError> {
        let value = parse_fixed_lower_hex(
            "asset_orchard_point",
            value.into(),
            ASSET_ORCHARD_POINT_BYTES,
        )?;
        parse_pallas_point(&value)?;
        Ok(Self(value))
    }

    pub fn from_affine(point: pallas::Affine) -> Result<Self, AssetOrchardError> {
        Ok(Self(bytes_to_hex(&point_enc(point)?)))
    }

    pub fn to_affine(&self) -> Result<pallas::Affine, AssetOrchardError> {
        parse_pallas_point(&self.0)
    }

    pub fn to_spend_auth_verification_key(
        &self,
    ) -> Result<
        orchard::primitives::redpallas::VerificationKey<orchard::primitives::redpallas::SpendAuth>,
        AssetOrchardError,
    > {
        orchard::primitives::redpallas::VerificationKey::try_from(fixed_lower_hex_array::<
            ASSET_ORCHARD_POINT_BYTES,
        >(
            "asset_orchard_randomized_verification_key",
            &self.0,
        )?)
        .map_err(|_| {
            AssetOrchardError::new(
                "invalid_asset_orchard_randomized_verification_key",
                "asset-orchard randomized verification key is not a valid RedPallas spend-auth key",
            )
        })
    }

    pub fn fields(&self) -> Result<RandomizedVerificationKeyFields, AssetOrchardError> {
        RandomizedVerificationKeyFields::from_affine(self.to_affine()?)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AssetOrchardPoint {
    type Error = AssetOrchardError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<AssetOrchardPoint> for String {
    fn from(value: AssetOrchardPoint) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPublicNoteOpening {
    pub diversifier: String,
    pub g_d: AssetOrchardPoint,
    pub pk_d: AssetOrchardPoint,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_lo: u128,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_hi: u128,
    pub value: u64,
    pub rho: AssetOrchardFieldElement,
    pub psi: AssetOrchardFieldElement,
    pub rcm: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardWalletNote {
    pub schema: String,
    pub pool_id: String,
    pub pool_domain: AssetOrchardFieldElement,
    pub asset_id: String,
    pub value: u64,
    pub output_commitment: AssetOrchardFieldElement,
    pub note: AssetOrchardPublicNoteOpening,
    pub nk: AssetOrchardSecret<AssetOrchardFieldElement>,
    pub rivk: AssetOrchardSecret<String>,
    pub spend_auth_signing_key: AssetOrchardSecret<String>,
    pub rseed: AssetOrchardSecret<String>,
}

impl AssetOrchardPublicNoteOpening {
    pub fn from_note(note: &AssetNoteOpening) -> Result<Self, AssetOrchardError> {
        note.validate()?;
        Ok(Self {
            diversifier: bytes_to_hex(&note.diversifier),
            g_d: AssetOrchardPoint::from_affine(note.g_d)?,
            pk_d: AssetOrchardPoint::from_affine(note.pk_d)?,
            asset_tag_lo: note.asset_tag.lo,
            asset_tag_hi: note.asset_tag.hi,
            value: note.value,
            rho: AssetOrchardFieldElement::from_field(note.rho),
            psi: AssetOrchardFieldElement::from_field(note.psi),
            rcm: bytes_to_hex(&scalar_enc(note.rcm)),
        })
    }

    pub fn validate_for_asset(&self, asset_id: &str, value: u64) -> Result<(), AssetOrchardError> {
        if self.value != value {
            return Err(AssetOrchardError::new(
                "asset_orchard_ingress_value_mismatch",
                "asset-orchard public note value does not match ingress amount",
            ));
        }
        let expected_tag = AssetTag::derive(asset_id)?;
        if self.asset_tag_lo != expected_tag.lo || self.asset_tag_hi != expected_tag.hi {
            return Err(AssetOrchardError::new(
                "asset_orchard_ingress_asset_tag_mismatch",
                "asset-orchard public note asset tag does not match asset_id",
            ));
        }
        self.to_note_opening()?.validate()
    }

    pub fn to_note_opening(&self) -> Result<AssetNoteOpening, AssetOrchardError> {
        let diversifier = fixed_lower_hex_array::<ASSET_ORCHARD_DIVERSIFIER_BYTES>(
            "asset_orchard_note_diversifier",
            &self.diversifier,
        )?;
        let note = AssetNoteOpening {
            diversifier,
            g_d: self.g_d.to_affine()?,
            pk_d: self.pk_d.to_affine()?,
            asset_tag: AssetTag {
                lo: self.asset_tag_lo,
                hi: self.asset_tag_hi,
            },
            value: self.value,
            rho: self.rho.to_field()?,
            psi: self.psi.to_field()?,
            rcm: parse_pallas_scalar(&self.rcm)?,
        };
        note.validate()?;
        Ok(note)
    }

    pub fn cmx(
        &self,
        pool_domain: pallas::Base,
    ) -> Result<AssetOrchardFieldElement, AssetOrchardError> {
        Ok(AssetOrchardFieldElement::from_field(
            self.to_note_opening()?.cmx(pool_domain)?,
        ))
    }
}

pub fn build_asset_orchard_wallet_note(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    asset_id: &str,
    value: u64,
    seed_hex: &str,
) -> Result<AssetOrchardWalletNote, AssetOrchardError> {
    build_asset_orchard_wallet_note_inner(
        chain_id,
        genesis_hash,
        protocol_version,
        asset_id,
        value,
        seed_hex,
        None,
    )
}

pub fn build_asset_orchard_wallet_note_with_rho(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    asset_id: &str,
    value: u64,
    seed_hex: &str,
    rho: pallas::Base,
) -> Result<AssetOrchardWalletNote, AssetOrchardError> {
    build_asset_orchard_wallet_note_inner(
        chain_id,
        genesis_hash,
        protocol_version,
        asset_id,
        value,
        seed_hex,
        Some(rho),
    )
}

fn build_asset_orchard_wallet_note_inner(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    asset_id: &str,
    value: u64,
    seed_hex: &str,
    rho_override: Option<pallas::Base>,
) -> Result<AssetOrchardWalletNote, AssetOrchardError> {
    let seed = fixed_lower_hex_array::<32>("asset_orchard_wallet_note_seed", seed_hex)?;
    let pool =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    let mut diversifier = [0u8; ASSET_ORCHARD_DIVERSIFIER_BYTES];
    diversifier.copy_from_slice(
        &derive_bytes("postfiat.asset_orchard.wallet_note.diversifier.v1", &seed)?
            [..ASSET_ORCHARD_DIVERSIFIER_BYTES],
    );
    let rseed = derive_bytes("postfiat.asset_orchard.wallet_note.rseed.v1", &seed)?;
    let rho = rho_override.unwrap_or(hash_to_pallas_base(
        "postfiat.asset_orchard.wallet_note.rho.v1",
        &seed,
    )?);
    let nk = hash_to_pallas_base("postfiat.asset_orchard.wallet_note.nk.v1", &seed)?;
    let rivk = hash_to_pallas_scalar_nonzero("postfiat.asset_orchard.wallet_note.rivk.v1", &seed)?;
    let spend_scalar = hash_to_pallas_scalar_nonzero(
        "postfiat.asset_orchard.wallet_note.spend_auth_sk.v1",
        &seed,
    )?;
    let spend_key = SigningKey::<SpendAuth>::try_from(scalar_enc(spend_scalar)).map_err(|_| {
        AssetOrchardError::new(
            "invalid_asset_orchard_spend_key",
            "derived AssetOrchard spend key is not accepted by RedPallas",
        )
    })?;
    let ak = verification_key_affine(&VerificationKey::from(&spend_key))?;
    let g_d = pallas::Point::hash_to_curve("postfiat.asset_orchard.wallet_note.g_d.v1")(&seed)
        .to_affine();
    let ivk = orchard_commit_ivk(ak, nk, rivk)?;
    let ivk_scalar = Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(ivk.to_repr()))
        .ok_or_else(|| {
            AssetOrchardError::new(
                "invalid_asset_orchard_ivk_scalar",
                "derived Orchard ivk cannot be represented as a Pallas scalar",
            )
        })?;
    let pk_d = (pallas::Point::from(g_d) * ivk_scalar).to_affine();
    let note = AssetNoteOpening {
        diversifier,
        g_d,
        pk_d,
        asset_tag: AssetTag::derive(asset_id)?,
        value,
        rho,
        psi: orchard_psi(&rseed, rho)?,
        rcm: orchard_rcm(&rseed, rho)?,
    };
    let output_commitment = AssetOrchardFieldElement::from_field(note.cmx(pool)?);
    let public_note = AssetOrchardPublicNoteOpening::from_note(&note)?;
    Ok(AssetOrchardWalletNote {
        schema: "postfiat-asset-orchard-wallet-note-v1".to_string(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        pool_domain: AssetOrchardFieldElement::from_field(pool),
        asset_id: asset_id.to_string(),
        value,
        output_commitment,
        note: public_note,
        nk: AssetOrchardSecret::new(AssetOrchardFieldElement::from_field(nk)),
        rivk: AssetOrchardSecret::new(bytes_to_hex(&scalar_enc(rivk))),
        spend_auth_signing_key: AssetOrchardSecret::new(bytes_to_hex(&scalar_enc(spend_scalar))),
        rseed: AssetOrchardSecret::new(bytes_to_hex(&rseed)),
    })
}

pub(crate) fn asset_orchard_incoming_viewing_key_from_seed(
    seed_hex: &str,
) -> Result<pallas::Scalar, AssetOrchardError> {
    let seed = fixed_lower_hex_array::<32>("asset_orchard_wallet_note_seed", seed_hex)?;
    let nk = hash_to_pallas_base("postfiat.asset_orchard.wallet_note.nk.v1", &seed)?;
    let rivk = hash_to_pallas_scalar_nonzero("postfiat.asset_orchard.wallet_note.rivk.v1", &seed)?;
    let spend_scalar = hash_to_pallas_scalar_nonzero(
        "postfiat.asset_orchard.wallet_note.spend_auth_sk.v1",
        &seed,
    )?;
    let spend_key = SigningKey::<SpendAuth>::try_from(scalar_enc(spend_scalar)).map_err(|_| {
        AssetOrchardError::new(
            "invalid_asset_orchard_spend_key",
            "derived AssetOrchard spend key is not accepted by RedPallas",
        )
    })?;
    let ak = verification_key_affine(&VerificationKey::from(&spend_key))?;
    let ivk = orchard_commit_ivk(ak, nk, rivk)?;
    Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(ivk.to_repr())).ok_or_else(|| {
        AssetOrchardError::new(
            "invalid_asset_orchard_ivk_scalar",
            "derived Orchard ivk cannot be represented as a Pallas scalar",
        )
    })
}

fn verification_key_affine(
    key: &VerificationKey<SpendAuth>,
) -> Result<pallas::Affine, AssetOrchardError> {
    let bytes = <[u8; 32]>::from(key);
    let point =
        Option::<pallas::Affine>::from(pallas::Affine::from_bytes(&bytes)).ok_or_else(|| {
            AssetOrchardError::new(
                "invalid_asset_orchard_verification_key",
                "derived spend verification key is not a canonical Pallas point",
            )
        })?;
    reject_identity_point("asset_orchard_spend_verification_key", point)?;
    Ok(point)
}

fn derive_bytes(dst: &str, seed: &[u8; 32]) -> Result<[u8; 32], AssetOrchardError> {
    validate_canonical_text("asset_orchard_derive_dst", dst, 256)?;
    let mut hasher = Sha3_512::new();
    Digest::update(&mut hasher, b"postfiat.asset_orchard.derive_bytes.v1");
    append_len_bytes(&mut hasher, dst.as_bytes())?;
    append_len_bytes(&mut hasher, seed)?;
    let digest = hasher.finalize();
    Ok(digest[0..32].try_into().expect("sha3-512 digest slice"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetOrchardSwapBindingHash(String);

impl AssetOrchardSwapBindingHash {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, AssetOrchardError> {
        Ok(Self(parse_fixed_lower_hex(
            "asset_orchard_swap_binding_hash",
            value.into(),
            ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES,
        )?))
    }

    pub fn from_bytes(bytes: &[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES]) -> Self {
        Self(bytes_to_hex(bytes))
    }

    pub fn to_bytes(
        &self,
    ) -> Result<[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES], AssetOrchardError> {
        fixed_lower_hex_array("asset_orchard_swap_binding_hash", &self.0)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AssetOrchardSwapBindingHash {
    type Error = AssetOrchardError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<AssetOrchardSwapBindingHash> for String {
    fn from(value: AssetOrchardSwapBindingHash) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetOrchardBoundedBytes {
    hex: String,
}

impl AssetOrchardBoundedBytes {
    pub fn parse_hex(
        label: &'static str,
        value: impl Into<String>,
        max_bytes: usize,
    ) -> Result<Self, AssetOrchardError> {
        let value = value.into();
        parse_lower_hex(label, &value, 1, max_bytes)?;
        Ok(Self { hex: value })
    }

    pub fn from_bytes(bytes: &[u8], max_bytes: usize) -> Result<Self, AssetOrchardError> {
        if bytes.is_empty() {
            return Err(AssetOrchardError::new(
                "empty_blob",
                "asset-orchard byte blob must not be empty",
            ));
        }
        if bytes.len() > max_bytes {
            return Err(AssetOrchardError::new(
                "oversized_blob",
                format!(
                    "asset-orchard byte blob has {} bytes, max {max_bytes}",
                    bytes.len()
                ),
            ));
        }
        Ok(Self {
            hex: bytes_to_hex(bytes),
        })
    }

    pub fn as_hex(&self) -> &str {
        &self.hex
    }

    pub fn byte_len(&self) -> usize {
        self.hex.len() / 2
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, AssetOrchardError> {
        hex_to_bytes(&self.hex)
            .map_err(|error| AssetOrchardError::new("invalid_hex", error.to_string()))
    }
}

impl TryFrom<String> for AssetOrchardBoundedBytes {
    type Error = AssetOrchardError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(
            "asset_orchard_bounded_bytes",
            value,
            ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
        )
    }
}

impl From<AssetOrchardBoundedBytes> for String {
    fn from(value: AssetOrchardBoundedBytes) -> Self {
        value.hex
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetOrchardProofBytes(AssetOrchardBoundedBytes);

impl AssetOrchardProofBytes {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, AssetOrchardError> {
        AssetOrchardBoundedBytes::parse_hex(
            "asset_orchard_proof",
            value,
            ASSET_ORCHARD_PROOF_MAX_BYTES,
        )
        .map(Self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AssetOrchardError> {
        AssetOrchardBoundedBytes::from_bytes(bytes, ASSET_ORCHARD_PROOF_MAX_BYTES).map(Self)
    }

    pub fn as_hex(&self) -> &str {
        self.0.as_hex()
    }

    pub fn byte_len(&self) -> usize {
        self.0.byte_len()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, AssetOrchardError> {
        self.0.to_bytes()
    }
}

impl TryFrom<String> for AssetOrchardProofBytes {
    type Error = AssetOrchardError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<AssetOrchardProofBytes> for String {
    fn from(value: AssetOrchardProofBytes) -> Self {
        value.0.hex
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetOrchardSpendAuthSignature(String);

impl AssetOrchardSpendAuthSignature {
    pub fn parse_hex(value: impl Into<String>) -> Result<Self, AssetOrchardError> {
        Ok(Self(parse_fixed_lower_hex(
            "asset_orchard_spend_authorization_signature",
            value.into(),
            ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES,
        )?))
    }

    pub fn from_orchard(
        signature: &orchard::primitives::redpallas::Signature<
            orchard::primitives::redpallas::SpendAuth,
        >,
    ) -> Self {
        let bytes: [u8; ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES] = signature.into();
        Self(bytes_to_hex(&bytes))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }

    pub fn to_orchard(
        &self,
    ) -> Result<
        orchard::primitives::redpallas::Signature<orchard::primitives::redpallas::SpendAuth>,
        AssetOrchardError,
    > {
        Ok(orchard::primitives::redpallas::Signature::from(
            fixed_lower_hex_array::<ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES>(
                "asset_orchard_spend_authorization_signature",
                &self.0,
            )?,
        ))
    }
}

impl TryFrom<String> for AssetOrchardSpendAuthSignature {
    type Error = AssetOrchardError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(value)
    }
}

impl From<AssetOrchardSpendAuthSignature> for String {
    fn from(value: AssetOrchardSpendAuthSignature) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardSwapAction {
    pub version: u16,
    pub schema: String,
    pub pool_id: String,
    pub proof_system_id: String,
    pub circuit_id: String,
    pub pool_domain: AssetOrchardFieldElement,
    pub anchor: AssetOrchardFieldElement,
    pub nullifiers: Vec<AssetOrchardFieldElement>,
    pub randomized_verification_keys: Vec<AssetOrchardPoint>,
    pub output_commitments: Vec<AssetOrchardFieldElement>,
    pub encrypted_outputs: Vec<AssetOrchardBoundedBytes>,
    pub accounting_inputs: Vec<AssetOrchardSwapAccountingRecord>,
    pub accounting_outputs: Vec<AssetOrchardSwapAccountingRecord>,
    pub pricing_claim: AssetOrchardPricingClaim,
    pub swap_binding_hash: AssetOrchardSwapBindingHash,
    pub fee: u64,
    pub proof: AssetOrchardProofBytes,
    pub spend_authorization_signatures: Vec<AssetOrchardSpendAuthSignature>,
}

/// Canonical public pricing statement whose ratio is constrained against the
/// private swap legs by the Asset-Orchard circuit. Asset tags disclose only
/// the protocol's one-way asset identifiers, not the underlying asset names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPricingClaim {
    pub nav_epoch: u64,
    pub reserve_packet_hash: String,
    pub ratio_numerator: u64,
    pub ratio_denominator: u64,
    pub mode: String,
    pub band_bps: u16,
    #[serde(with = "u128_hex_serde")]
    pub base_asset_tag_lo: u128,
    #[serde(with = "u128_hex_serde")]
    pub base_asset_tag_hi: u128,
    #[serde(with = "u128_hex_serde")]
    pub quote_asset_tag_lo: u128,
    #[serde(with = "u128_hex_serde")]
    pub quote_asset_tag_hi: u128,
}

impl AssetOrchardPricingClaim {
    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        if self.nav_epoch == 0 || self.ratio_numerator == 0 || self.ratio_denominator == 0 {
            return Err(AssetOrchardError::new(
                "invalid_asset_orchard_pricing_claim",
                "pricing epoch and ratio terms must be nonzero",
            ));
        }
        fixed_lower_hex_array::<48>(
            "asset_orchard_pricing_reserve_packet_hash",
            &self.reserve_packet_hash,
        )?;
        if !matches!(
            self.mode.as_str(),
            "at_nav" | "at_nav_with_band" | "negotiated"
        ) {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_pricing_mode",
                format!("unsupported pricing mode `{}`", self.mode),
            ));
        }
        if self.band_bps > 10_000 {
            return Err(AssetOrchardError::new(
                "invalid_asset_orchard_pricing_band",
                "pricing band must not exceed 10000 bps",
            ));
        }
        AssetTag {
            lo: self.base_asset_tag_lo,
            hi: self.base_asset_tag_hi,
        }
        .validate()?;
        AssetTag {
            lo: self.quote_asset_tag_lo,
            hi: self.quote_asset_tag_hi,
        }
        .validate()?;
        Ok(())
    }

    pub fn commitment_fields(&self) -> Result<[pallas::Base; 3], AssetOrchardError> {
        self.validate()?;
        let mut hasher = Sha3_384::new();
        Digest::update(&mut hasher, b"postfiat.asset_orchard.pricing_claim.v1");
        Digest::update(&mut hasher, self.nav_epoch.to_le_bytes());
        Digest::update(
            &mut hasher,
            hex_to_bytes(&self.reserve_packet_hash).map_err(|e| {
                AssetOrchardError::new(
                    "invalid_asset_orchard_pricing_reserve_packet_hash",
                    e.to_string(),
                )
            })?,
        );
        Digest::update(&mut hasher, self.ratio_numerator.to_le_bytes());
        Digest::update(&mut hasher, self.ratio_denominator.to_le_bytes());
        append_len_bytes(&mut hasher, self.mode.as_bytes())?;
        Digest::update(&mut hasher, self.band_bps.to_le_bytes());
        for limb in [
            self.base_asset_tag_lo,
            self.base_asset_tag_hi,
            self.quote_asset_tag_lo,
            self.quote_asset_tag_hi,
        ] {
            Digest::update(&mut hasher, limb.to_le_bytes());
        }
        let digest = hasher.finalize();
        Ok(std::array::from_fn(|index| {
            let start = index * 16;
            pallas::Base::from_u128(u128::from_le_bytes(
                digest[start..start + 16]
                    .try_into()
                    .expect("fixed digest limb"),
            ))
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardSwapAccountingRecord {
    pub output_commitment: String,
    pub value_commitment: AssetOrchardPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressAction {
    pub version: u16,
    pub schema: String,
    pub pool_id: String,
    pub proof_system_id: String,
    pub circuit_id: String,
    pub pool_domain: AssetOrchardFieldElement,
    pub anchor: AssetOrchardFieldElement,
    pub nullifier: AssetOrchardFieldElement,
    pub randomized_verification_key: AssetOrchardPoint,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_lo: u128,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_hi: u128,
    pub amount: u64,
    pub fee: u64,
    pub exit_binding_hash: AssetOrchardSwapBindingHash,
    pub proof: AssetOrchardProofBytes,
    pub spend_authorization_signature: AssetOrchardSpendAuthSignature,
}

impl AssetOrchardSwapAction {
    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        if self.version != ASSET_ORCHARD_ACTION_VERSION_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_version",
                format!("unsupported asset-orchard swap version {}", self.version),
            ));
        }
        if self.schema != ASSET_ORCHARD_ACTION_SCHEMA_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_schema",
                format!("unsupported asset-orchard schema `{}`", self.schema),
            ));
        }
        if self.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_pool",
                format!("unsupported asset-orchard pool `{}`", self.pool_id),
            ));
        }
        if self.proof_system_id != ASSET_ORCHARD_PROOF_SYSTEM_ID_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_proof_system",
                format!(
                    "unsupported asset-orchard proof system `{}`",
                    self.proof_system_id
                ),
            ));
        }
        supported_asset_orchard_swap_circuit_id(&self.circuit_id)?;
        if self.fee != 0 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_fee",
                "asset-orchard swap v1 requires fee 0",
            ));
        }
        self.validate_index_counts()?;
        if self.proof.byte_len() > ASSET_ORCHARD_PROOF_MAX_BYTES {
            return Err(AssetOrchardError::new(
                "oversized_asset_orchard_proof",
                format!(
                    "asset-orchard proof has {} bytes, max {ASSET_ORCHARD_PROOF_MAX_BYTES}",
                    self.proof.byte_len()
                ),
            ));
        }
        if has_duplicate_hex(self.nullifiers.iter().map(AssetOrchardFieldElement::as_hex)) {
            return Err(AssetOrchardError::new(
                "duplicate_nullifier",
                "asset-orchard swap contains duplicate nullifiers",
            ));
        }
        if has_duplicate_hex(
            self.output_commitments
                .iter()
                .map(AssetOrchardFieldElement::as_hex),
        ) {
            return Err(AssetOrchardError::new(
                "duplicate_output_commitment",
                "asset-orchard swap contains duplicate output commitments",
            ));
        }
        self.validate_accounting_records()?;
        self.pricing_claim.validate()?;
        for output in &self.encrypted_outputs {
            if output.byte_len() > ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES {
                return Err(AssetOrchardError::new(
                    "oversized_encrypted_output",
                    format!(
                        "encrypted output has {} bytes, max {ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES}",
                        output.byte_len()
                    ),
                ));
            }
        }
        let computed = AssetOrchardSwapBindingHash::from_bytes(&swap_binding_hash(
            &self.public_fields_without_binding_check()?,
        )?);
        if computed != self.swap_binding_hash {
            return Err(AssetOrchardError::new(
                "asset_orchard_swap_binding_mismatch",
                "serialized swap_binding_hash does not match recomputed H_action",
            ));
        }
        Ok(())
    }

    pub fn public_fields(&self) -> Result<AssetOrchardActionPublicFields, AssetOrchardError> {
        self.validate()?;
        self.public_fields_without_binding_check()
    }

    pub fn public_instance(
        &self,
    ) -> Result<[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN], AssetOrchardError> {
        self.public_fields()?.public_instance()
    }

    pub fn sighash(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<[u8; ASSET_ORCHARD_SIGHASH_BYTES], AssetOrchardError> {
        self.validate_index_counts()?;
        self.validate()?;
        let rks = [
            self.randomized_verification_keys[0].to_affine()?,
            self.randomized_verification_keys[1].to_affine()?,
        ];
        let encrypted_outputs = [
            self.encrypted_outputs[0].to_bytes()?,
            self.encrypted_outputs[1].to_bytes()?,
        ];
        h_sig(&AssetOrchardSigPreimage {
            chain_id,
            genesis_hash,
            protocol_version,
            pool_id: &self.pool_id,
            circuit_id: supported_asset_orchard_swap_circuit_id(&self.circuit_id)?,
            pool_domain: self.pool_domain.to_field()?,
            anchor: self.anchor.to_field()?,
            nullifiers: [
                self.nullifiers[0].to_field()?,
                self.nullifiers[1].to_field()?,
            ],
            randomized_verification_keys: rks,
            output_commitments: [
                self.output_commitments[0].to_field()?,
                self.output_commitments[1].to_field()?,
            ],
            encrypted_outputs: [
                encrypted_outputs[0].as_slice(),
                encrypted_outputs[1].as_slice(),
            ],
            accounting_inputs: [&self.accounting_inputs[0], &self.accounting_inputs[1]],
            accounting_outputs: [&self.accounting_outputs[0], &self.accounting_outputs[1]],
            swap_binding_hash: self.swap_binding_hash.to_bytes()?,
            fee: self.fee,
        })
    }

    pub fn expected_pool_domain(
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<pallas::Base, AssetOrchardError> {
        pool_domain(&PoolDomainInput {
            chain_id,
            genesis_hash,
            protocol_version,
            pool_id: ASSET_ORCHARD_POOL_ID_V1,
            note_version: ASSET_ORCHARD_NOTE_VERSION_V1,
        })
    }

    pub fn validate_domain_binding(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<(), AssetOrchardError> {
        let expected = Self::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
        if self.pool_domain.to_field()? != expected {
            return Err(AssetOrchardError::new(
                "asset_orchard_pool_domain_mismatch",
                "serialized pool_domain does not match local chain/genesis/protocol/pool",
            ));
        }
        Ok(())
    }

    pub fn verify_spend_authorizations(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<(), AssetOrchardError> {
        self.validate_index_counts()?;
        self.validate_domain_binding(chain_id, genesis_hash, protocol_version)?;
        let sighash = self.sighash(chain_id, genesis_hash, protocol_version)?;
        for index in 0..ASSET_ORCHARD_LEG_COUNT {
            let rk = self.randomized_verification_keys[index].to_spend_auth_verification_key()?;
            let signature = self.spend_authorization_signatures[index].to_orchard()?;
            rk.verify(&sighash, &signature).map_err(|_| {
                AssetOrchardError::new(
                    "asset_orchard_spend_authorization_failed",
                    format!("asset-orchard spend authorization signature {index} failed"),
                )
            })?;
        }
        Ok(())
    }

    fn public_fields_without_binding_check(
        &self,
    ) -> Result<AssetOrchardActionPublicFields, AssetOrchardError> {
        self.validate_index_counts()?;
        let encrypted_outputs = [
            self.encrypted_outputs[0].to_bytes()?,
            self.encrypted_outputs[1].to_bytes()?,
        ];
        Ok(AssetOrchardActionPublicFields {
            pool_domain: self.pool_domain.to_field()?,
            anchor: self.anchor.to_field()?,
            nullifiers: [
                self.nullifiers[0].to_field()?,
                self.nullifiers[1].to_field()?,
            ],
            randomized_verification_keys: [
                self.randomized_verification_keys[0].fields()?,
                self.randomized_verification_keys[1].fields()?,
            ],
            output_commitments: [
                self.output_commitments[0].to_field()?,
                self.output_commitments[1].to_field()?,
            ],
            encrypted_output_hashes: [
                encrypted_output_hash(0, &encrypted_outputs[0])?,
                encrypted_output_hash(1, &encrypted_outputs[1])?,
            ],
            pricing: AssetOrchardPricingPublicFields {
                base_asset_tag: AssetTag {
                    lo: self.pricing_claim.base_asset_tag_lo,
                    hi: self.pricing_claim.base_asset_tag_hi,
                },
                quote_asset_tag: AssetTag {
                    lo: self.pricing_claim.quote_asset_tag_lo,
                    hi: self.pricing_claim.quote_asset_tag_hi,
                },
                ratio_numerator: self.pricing_claim.ratio_numerator,
                ratio_denominator: self.pricing_claim.ratio_denominator,
                commitment: self.pricing_claim.commitment_fields()?,
            },
            fee: self.fee,
        })
    }

    fn validate_index_counts(&self) -> Result<(), AssetOrchardError> {
        validate_count("nullifiers", self.nullifiers.len(), ASSET_ORCHARD_LEG_COUNT)?;
        validate_count(
            "randomized_verification_keys",
            self.randomized_verification_keys.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_count(
            "output_commitments",
            self.output_commitments.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_count(
            "encrypted_outputs",
            self.encrypted_outputs.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_count(
            "spend_authorization_signatures",
            self.spend_authorization_signatures.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )
    }

    fn validate_accounting_records(&self) -> Result<(), AssetOrchardError> {
        validate_count(
            "accounting_inputs",
            self.accounting_inputs.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_count(
            "accounting_outputs",
            self.accounting_outputs.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_accounting_record_set("accounting input", &self.accounting_inputs)?;
        validate_accounting_record_set("accounting output", &self.accounting_outputs)?;
        for (index, (record, commitment)) in self
            .accounting_outputs
            .iter()
            .zip(self.output_commitments.iter())
            .enumerate()
        {
            if record.output_commitment != commitment.as_hex() {
                return Err(AssetOrchardError::new(
                    "asset_orchard_accounting_output_commitment_mismatch",
                    format!(
                        "asset-orchard accounting output {index} commitment does not match proof output commitment"
                    ),
                ));
            }
        }
        let input_sum = accounting_sum(&self.accounting_inputs)?;
        let output_sum = accounting_sum(&self.accounting_outputs)?;
        if input_sum != output_sum {
            return Err(AssetOrchardError::new(
                "asset_orchard_accounting_not_conserved",
                "asset-orchard swap aggregate accounting commitment sum is not conserved",
            ));
        }
        Ok(())
    }
}

impl AssetOrchardPrivateEgressAction {
    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        if self.version != ASSET_ORCHARD_ACTION_VERSION_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_private_egress_version",
                format!(
                    "unsupported asset-orchard private egress version {}",
                    self.version
                ),
            ));
        }
        if self.schema != ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_private_egress_schema",
                format!(
                    "unsupported asset-orchard private egress schema `{}`",
                    self.schema
                ),
            ));
        }
        if self.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_pool",
                format!("unsupported asset-orchard pool `{}`", self.pool_id),
            ));
        }
        if self.proof_system_id != ASSET_ORCHARD_PROOF_SYSTEM_ID_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_proof_system",
                format!(
                    "unsupported asset-orchard proof system `{}`",
                    self.proof_system_id
                ),
            ));
        }
        supported_asset_orchard_private_egress_circuit_id(&self.circuit_id)?;
        if self.amount == 0 {
            return Err(AssetOrchardError::new(
                "zero_private_egress_amount",
                "asset-orchard private egress amount must be nonzero",
            ));
        }
        if self.fee != 0 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_private_egress_fee",
                "asset-orchard private egress v1 requires fee 0",
            ));
        }
        AssetTag {
            lo: self.asset_tag_lo,
            hi: self.asset_tag_hi,
        }
        .validate()?;
        if self.proof.byte_len() > ASSET_ORCHARD_PROOF_MAX_BYTES {
            return Err(AssetOrchardError::new(
                "oversized_asset_orchard_private_egress_proof",
                format!(
                    "asset-orchard private egress proof has {} bytes, max {ASSET_ORCHARD_PROOF_MAX_BYTES}",
                    self.proof.byte_len()
                ),
            ));
        }
        self.public_fields_without_binding_check()?
            .public_instance()?;
        Ok(())
    }

    pub fn public_fields(
        &self,
    ) -> Result<AssetOrchardPrivateEgressPublicFields, AssetOrchardError> {
        self.validate()?;
        self.public_fields_without_binding_check()
    }

    pub fn public_instance(
        &self,
    ) -> Result<[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN], AssetOrchardError>
    {
        self.public_fields()?.public_instance()
    }

    pub fn sighash(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
        to: &str,
        asset_id: &str,
        policy_id: &str,
        disclosure_hash: &str,
    ) -> Result<[u8; ASSET_ORCHARD_SIGHASH_BYTES], AssetOrchardError> {
        self.validate()?;
        asset_orchard_private_egress_sighash(&AssetOrchardPrivateEgressPreimage {
            chain_id,
            genesis_hash,
            protocol_version,
            pool_id: &self.pool_id,
            circuit_id: supported_asset_orchard_private_egress_circuit_id(&self.circuit_id)?,
            pool_domain: self.pool_domain.to_field()?,
            anchor: self.anchor.to_field()?,
            to,
            asset_id,
            amount: self.amount,
            fee: self.fee,
            policy_id,
            disclosure_hash,
            nullifier: self.nullifier.to_field()?,
            randomized_verification_key: self.randomized_verification_key.to_affine()?,
            asset_tag_lo: self.asset_tag_lo,
            asset_tag_hi: self.asset_tag_hi,
            exit_binding_hash: self.exit_binding_hash.to_bytes()?,
        })
    }

    pub fn validate_domain_binding(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<(), AssetOrchardError> {
        let expected =
            AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
        if self.pool_domain.to_field()? != expected {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_pool_domain_mismatch",
                "serialized pool_domain does not match local chain/genesis/protocol/pool",
            ));
        }
        Ok(())
    }

    pub fn verify_spend_authorization(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
        to: &str,
        asset_id: &str,
        policy_id: &str,
        disclosure_hash: &str,
    ) -> Result<(), AssetOrchardError> {
        self.validate_domain_binding(chain_id, genesis_hash, protocol_version)?;
        let sighash = self.sighash(
            chain_id,
            genesis_hash,
            protocol_version,
            to,
            asset_id,
            policy_id,
            disclosure_hash,
        )?;
        let rk = self
            .randomized_verification_key
            .to_spend_auth_verification_key()?;
        let signature = self.spend_authorization_signature.to_orchard()?;
        rk.verify(&sighash, &signature).map_err(|_| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_spend_authorization_failed",
                "asset-orchard private egress spend authorization signature failed",
            )
        })
    }

    fn public_fields_without_binding_check(
        &self,
    ) -> Result<AssetOrchardPrivateEgressPublicFields, AssetOrchardError> {
        Ok(AssetOrchardPrivateEgressPublicFields {
            pool_domain: self.pool_domain.to_field()?,
            anchor: self.anchor.to_field()?,
            nullifier: self.nullifier.to_field()?,
            randomized_verification_key: self.randomized_verification_key.fields()?,
            asset_tag: AssetTag {
                lo: self.asset_tag_lo,
                hi: self.asset_tag_hi,
            },
            amount: self.amount,
            fee: self.fee,
            exit_binding_hash: self.exit_binding_hash.to_bytes()?,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AssetTag {
    pub lo: u128,
    pub hi: u128,
}

impl AssetTag {
    pub fn derive(asset_id: &str) -> Result<Self, AssetOrchardError> {
        validate_canonical_text("asset_id", asset_id, ASSET_ORCHARD_MAX_ASSET_ID_BYTES)?;
        Self::derive_from_canonical_bytes(asset_id.as_bytes())
    }

    pub fn derive_from_canonical_bytes(
        canonical_asset_id: &[u8],
    ) -> Result<Self, AssetOrchardError> {
        validate_canonical_bytes(
            "asset_id",
            canonical_asset_id,
            ASSET_ORCHARD_MAX_ASSET_ID_BYTES,
        )?;
        let mut hasher = Sha3_384::new();
        Digest::update(&mut hasher, ASSET_TAG_DOMAIN);
        append_len_bytes(&mut hasher, canonical_asset_id)?;
        let digest = hasher.finalize();
        let lo = u128::from_le_bytes(
            digest[0..16]
                .try_into()
                .map_err(|_| AssetOrchardError::new("digest_slice", "invalid tag lo slice"))?,
        );
        let hi = u128::from_le_bytes(
            digest[16..32]
                .try_into()
                .map_err(|_| AssetOrchardError::new("digest_slice", "invalid tag hi slice"))?,
        );
        let tag = Self { lo, hi };
        tag.validate()?;
        Ok(tag)
    }

    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        if self.lo == 0 && self.hi == 0 {
            return Err(AssetOrchardError::new(
                "zero_asset_tag",
                "asset tag (0,0) is reserved and invalid",
            ));
        }
        Ok(())
    }

    pub fn as_fields(&self) -> [pallas::Base; 2] {
        [
            pallas::Base::from_u128(self.lo),
            pallas::Base::from_u128(self.hi),
        ]
    }

    pub fn to_le_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..16].copy_from_slice(&self.lo.to_le_bytes());
        bytes[16..32].copy_from_slice(&self.hi.to_le_bytes());
        bytes
    }
}

pub fn asset_orchard_accounting_value_commitment(
    tag: AssetTag,
    amount: u64,
    blinding: pallas::Scalar,
) -> Result<AssetOrchardPoint, AssetOrchardError> {
    tag.validate()?;
    if amount == 0 {
        return Err(AssetOrchardError::new(
            "zero_asset_orchard_accounting_value",
            "asset-orchard accounting amount must be nonzero",
        ));
    }
    let asset_generator = accounting_asset_generator(tag)?;
    let blinding_generator = pallas::Point::hash_to_curve(ACCOUNTING_VALUE_COMMITMENT_H_DST)(b"H");
    let point = asset_generator * pallas::Scalar::from(amount) + blinding_generator * blinding;
    AssetOrchardPoint::from_affine(point.to_affine())
}

fn accounting_asset_generator(tag: AssetTag) -> Result<pallas::Point, AssetOrchardError> {
    tag.validate()?;
    Ok(pallas::Point::hash_to_curve(
        ACCOUNTING_VALUE_COMMITMENT_G_DST,
    )(&tag.to_le_bytes()))
}

pub fn asset_orchard_accounting_record(
    output_commitment: &AssetOrchardFieldElement,
    tag: AssetTag,
    amount: u64,
    blinding: pallas::Scalar,
) -> Result<AssetOrchardSwapAccountingRecord, AssetOrchardError> {
    Ok(AssetOrchardSwapAccountingRecord {
        output_commitment: output_commitment.as_hex().to_string(),
        value_commitment: asset_orchard_accounting_value_commitment(tag, amount, blinding)?,
    })
}

pub fn asset_orchard_swap_accounting_records(
    input_notes: &[AssetOrchardWalletNote; ASSET_ORCHARD_LEG_COUNT],
    output_notes: &[AssetOrchardWalletNote; ASSET_ORCHARD_LEG_COUNT],
) -> Result<
    (
        Vec<AssetOrchardSwapAccountingRecord>,
        Vec<AssetOrchardSwapAccountingRecord>,
    ),
    AssetOrchardError,
> {
    let mut input_blindings = Vec::with_capacity(ASSET_ORCHARD_LEG_COUNT);
    let mut input_blinding_total = pallas::Scalar::ZERO;
    for (index, note) in input_notes.iter().enumerate() {
        let blinding = accounting_blinding_from_note("input", index, note)?;
        input_blindings.push(blinding);
        input_blinding_total += blinding;
    }

    let mut output_blindings = vec![pallas::Scalar::ZERO; ASSET_ORCHARD_LEG_COUNT];
    let mut output_blinding_total = pallas::Scalar::ZERO;
    for (index, note) in output_notes
        .iter()
        .enumerate()
        .take(ASSET_ORCHARD_LEG_COUNT.saturating_sub(1))
    {
        let blinding = accounting_blinding_from_note("output", index, note)?;
        output_blindings[index] = blinding;
        output_blinding_total += blinding;
    }
    output_blindings[ASSET_ORCHARD_LEG_COUNT - 1] = input_blinding_total - output_blinding_total;

    let accounting_inputs = input_notes
        .iter()
        .zip(input_blindings)
        .map(|(note, blinding)| {
            let tag = wallet_note_asset_tag(note)?;
            asset_orchard_accounting_record(&note.output_commitment, tag, note.value, blinding)
        })
        .collect::<Result<Vec<_>, AssetOrchardError>>()?;
    let accounting_outputs = output_notes
        .iter()
        .zip(output_blindings)
        .map(|(note, blinding)| {
            let tag = wallet_note_asset_tag(note)?;
            asset_orchard_accounting_record(&note.output_commitment, tag, note.value, blinding)
        })
        .collect::<Result<Vec<_>, AssetOrchardError>>()?;
    Ok((accounting_inputs, accounting_outputs))
}

pub fn asset_orchard_accounting_commitment_sum(
    records: &[AssetOrchardSwapAccountingRecord],
) -> Result<[u8; ASSET_ORCHARD_POINT_BYTES], AssetOrchardError> {
    accounting_sum(records)
}

fn wallet_note_asset_tag(note: &AssetOrchardWalletNote) -> Result<AssetTag, AssetOrchardError> {
    let tag = AssetTag {
        lo: note.note.asset_tag_lo,
        hi: note.note.asset_tag_hi,
    };
    tag.validate()?;
    Ok(tag)
}

fn accounting_blinding_from_note(
    role: &str,
    index: usize,
    note: &AssetOrchardWalletNote,
) -> Result<pallas::Scalar, AssetOrchardError> {
    validate_canonical_text("asset_orchard_accounting_role", role, 32)?;
    let mut payload = Vec::new();
    append_len_bytes_vec(&mut payload, role.as_bytes())?;
    payload.extend_from_slice(&(index as u32).to_le_bytes());
    append_len_bytes_vec(&mut payload, note.output_commitment.as_hex().as_bytes())?;
    append_len_bytes_vec(&mut payload, note.rseed.expose_secret().as_bytes())?;
    hash_to_field::<pallas::Scalar>(
        HASH_TO_PALLAS_SCALAR_DOMAIN,
        ACCOUNTING_BLINDING_DST,
        &payload,
    )
}

#[derive(Debug, Clone)]
pub struct PoolDomainInput<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: [u8; 32],
    pub protocol_version: u32,
    pub pool_id: &'a str,
    pub note_version: u16,
}

pub fn pool_domain(input: &PoolDomainInput<'_>) -> Result<pallas::Base, AssetOrchardError> {
    validate_canonical_text("chain_id", input.chain_id, 256)?;
    validate_canonical_text("pool_id", input.pool_id, ASSET_ORCHARD_MAX_POOL_ID_BYTES)?;
    if input.protocol_version == 0 {
        return Err(AssetOrchardError::new(
            "invalid_protocol_version",
            "protocol_version must be nonzero",
        ));
    }
    if input.note_version == 0 {
        return Err(AssetOrchardError::new(
            "invalid_note_version",
            "note_version must be nonzero",
        ));
    }

    let mut preimage = Vec::new();
    append_len_bytes_vec(&mut preimage, input.chain_id.as_bytes())?;
    preimage.extend_from_slice(&input.genesis_hash);
    preimage.extend_from_slice(&input.protocol_version.to_le_bytes());
    append_len_bytes_vec(&mut preimage, input.pool_id.as_bytes())?;
    preimage.extend_from_slice(&input.note_version.to_le_bytes());
    hash_to_pallas_base(POOL_DOMAIN_DST, &preimage)
}

pub fn hash_to_pallas_base(dst: &str, msg: &[u8]) -> Result<pallas::Base, AssetOrchardError> {
    validate_canonical_text("hash_dst", dst, 256)?;
    hash_to_field::<pallas::Base>(HASH_TO_PALLAS_BASE_DOMAIN, dst.as_bytes(), msg)
}

pub fn hash_to_pallas_scalar_nonzero(
    dst: &str,
    msg: &[u8],
) -> Result<pallas::Scalar, AssetOrchardError> {
    validate_canonical_text("hash_dst", dst, 256)?;
    loop_hash_to_scalar(dst.as_bytes(), msg)
}

pub fn random_pallas_scalar_nonzero() -> pallas::Scalar {
    loop {
        let scalar = pallas::Scalar::random(OsRng);
        if !bool::from(scalar.is_zero()) {
            return scalar;
        }
    }
}

pub fn const_field(name: &str) -> Result<pallas::Base, AssetOrchardError> {
    validate_canonical_text("const_field_name", name, 256)?;
    hash_to_pallas_base(CONST_FIELD_DST, name.as_bytes())
}

pub fn orchard_psi(
    rseed: &[u8; ASSET_ORCHARD_RSEED_BYTES],
    rho: pallas::Base,
) -> Result<pallas::Base, AssetOrchardError> {
    let mut msg = Vec::with_capacity(ASSET_ORCHARD_RSEED_BYTES + ASSET_ORCHARD_FIELD_BYTES);
    msg.extend_from_slice(rseed);
    msg.extend_from_slice(&field_enc(rho));
    hash_to_pallas_base(ORCHARD_PSI_DST, &msg)
}

pub fn orchard_rcm(
    rseed: &[u8; ASSET_ORCHARD_RSEED_BYTES],
    rho: pallas::Base,
) -> Result<pallas::Scalar, AssetOrchardError> {
    let mut msg = Vec::with_capacity(ASSET_ORCHARD_RSEED_BYTES + ASSET_ORCHARD_FIELD_BYTES);
    msg.extend_from_slice(rseed);
    msg.extend_from_slice(&field_enc(rho));
    hash_to_pallas_scalar_nonzero(ORCHARD_RCM_DST, &msg)
}

pub fn orchard_commit_ivk(
    ak: pallas::Affine,
    nk: pallas::Base,
    rivk: pallas::Scalar,
) -> Result<pallas::Base, AssetOrchardError> {
    let ak_x = *Option::<Coordinates<pallas::Affine>>::from(ak.coordinates())
        .ok_or_else(|| AssetOrchardError::new("invalid_ak", "ak must not be identity"))?
        .x();
    let domain = sinsemilla::CommitDomain::new("z.cash:Orchard-CommitIvk");
    Option::<pallas::Base>::from(
        domain.short_commit(
            ak_x.to_le_bits()
                .iter()
                .by_vals()
                .take(pallas::Base::NUM_BITS as usize)
                .chain(
                    nk.to_le_bits()
                        .iter()
                        .by_vals()
                        .take(pallas::Base::NUM_BITS as usize),
                ),
            &rivk,
        ),
    )
    .ok_or_else(|| {
        AssetOrchardError::new(
            "orchard_commit_ivk_failed",
            "Orchard CommitIvk returned an invalid field element",
        )
    })
}

pub fn asset_derive_nullifier(
    pool_domain: pallas::Base,
    nk: pallas::Base,
    rho: pallas::Base,
    psi: pallas::Base,
    cmx: pallas::Base,
) -> Result<pallas::Base, AssetOrchardError> {
    poseidon_hash1(
        NULLIFIER_HASH_NAME,
        &asset_derive_nullifier_fields(pool_domain, nk, rho, psi, cmx)?,
    )
}

pub fn asset_derive_nullifier_fields(
    pool_domain: pallas::Base,
    nk: pallas::Base,
    rho: pallas::Base,
    psi: pallas::Base,
    cmx: pallas::Base,
) -> Result<[pallas::Base; 6], AssetOrchardError> {
    Ok([
        const_field(NOTE_VERSION_CONST_NAME)?,
        pool_domain,
        nk,
        rho,
        psi,
        cmx,
    ])
}

pub fn asset_derive_nullifier_poseidon_inputs(
    pool_domain: pallas::Base,
    nk: pallas::Base,
    rho: pallas::Base,
    psi: pallas::Base,
    cmx: pallas::Base,
) -> Result<[pallas::Base; 8], AssetOrchardError> {
    let fields = asset_derive_nullifier_fields(pool_domain, nk, rho, psi, cmx)?;
    Ok([
        const_field(NULLIFIER_HASH_NAME)?,
        pallas::Base::from(fields.len() as u64),
        fields[0],
        fields[1],
        fields[2],
        fields[3],
        fields[4],
        fields[5],
    ])
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RandomizedVerificationKeyFields {
    pub x: pallas::Base,
    pub y: pallas::Base,
}

impl RandomizedVerificationKeyFields {
    pub fn from_affine(point: pallas::Affine) -> Result<Self, AssetOrchardError> {
        let coordinates: Coordinates<pallas::Affine> = Option::from(point.coordinates())
            .ok_or_else(|| {
                AssetOrchardError::new(
                    "invalid_randomized_verification_key",
                    "randomized verification key must not be identity",
                )
            })?;
        Ok(Self {
            x: *coordinates.x(),
            y: *coordinates.y(),
        })
    }
}

pub fn asset_output_rho(
    pool_domain: pallas::Base,
    anchor: pallas::Base,
    nullifiers: [pallas::Base; 2],
    rks: [RandomizedVerificationKeyFields; 2],
    output_index: u8,
) -> Result<pallas::Base, AssetOrchardError> {
    poseidon_hash1(
        OUTPUT_RHO_HASH_NAME,
        &asset_output_rho_fields(pool_domain, anchor, nullifiers, rks, output_index)?,
    )
}

pub fn asset_output_rho_fields(
    pool_domain: pallas::Base,
    anchor: pallas::Base,
    nullifiers: [pallas::Base; 2],
    rks: [RandomizedVerificationKeyFields; 2],
    output_index: u8,
) -> Result<[pallas::Base; 9], AssetOrchardError> {
    if output_index > 1 {
        return Err(AssetOrchardError::new(
            "invalid_output_index",
            "asset-orchard swap output index must be 0 or 1",
        ));
    }
    Ok([
        pool_domain,
        anchor,
        nullifiers[0],
        nullifiers[1],
        rks[0].x,
        rks[0].y,
        rks[1].x,
        rks[1].y,
        pallas::Base::from(output_index as u64),
    ])
}

pub fn asset_output_rho_poseidon_inputs(
    pool_domain: pallas::Base,
    anchor: pallas::Base,
    nullifiers: [pallas::Base; 2],
    rks: [RandomizedVerificationKeyFields; 2],
    output_index: u8,
) -> Result<[pallas::Base; 11], AssetOrchardError> {
    let fields = asset_output_rho_fields(pool_domain, anchor, nullifiers, rks, output_index)?;
    Ok([
        const_field(OUTPUT_RHO_HASH_NAME)?,
        pallas::Base::from(fields.len() as u64),
        fields[0],
        fields[1],
        fields[2],
        fields[3],
        fields[4],
        fields[5],
        fields[6],
        fields[7],
        fields[8],
    ])
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct EncryptedOutputHash {
    pub limbs: [u128; 3],
}

impl EncryptedOutputHash {
    pub fn as_fields(&self) -> [pallas::Base; 3] {
        [
            pallas::Base::from_u128(self.limbs[0]),
            pallas::Base::from_u128(self.limbs[1]),
            pallas::Base::from_u128(self.limbs[2]),
        ]
    }
}

pub fn encrypted_output_hash(
    output_index: u8,
    encrypted_output: &[u8],
) -> Result<EncryptedOutputHash, AssetOrchardError> {
    if output_index > 1 {
        return Err(AssetOrchardError::new(
            "invalid_output_index",
            "encrypted output index must be 0 or 1",
        ));
    }
    let mut hasher = Sha3_384::new();
    Digest::update(&mut hasher, ENCRYPTED_OUTPUT_HASH_DOMAIN);
    Digest::update(&mut hasher, [output_index]);
    append_len_bytes(&mut hasher, encrypted_output)?;
    let digest = hasher.finalize();
    Ok(EncryptedOutputHash {
        limbs: [
            u128::from_le_bytes(digest[0..16].try_into().map_err(|_| {
                AssetOrchardError::new("digest_slice", "invalid encrypted-output limb 0")
            })?),
            u128::from_le_bytes(digest[16..32].try_into().map_err(|_| {
                AssetOrchardError::new("digest_slice", "invalid encrypted-output limb 1")
            })?),
            u128::from_le_bytes(digest[32..48].try_into().map_err(|_| {
                AssetOrchardError::new("digest_slice", "invalid encrypted-output limb 2")
            })?),
        ],
    })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AssetOrchardPricingPublicFields {
    pub base_asset_tag: AssetTag,
    pub quote_asset_tag: AssetTag,
    pub ratio_numerator: u64,
    pub ratio_denominator: u64,
    pub commitment: [pallas::Base; 3],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AssetOrchardActionPublicFields {
    pub pool_domain: pallas::Base,
    pub anchor: pallas::Base,
    pub nullifiers: [pallas::Base; 2],
    pub randomized_verification_keys: [RandomizedVerificationKeyFields; 2],
    pub output_commitments: [pallas::Base; 2],
    pub encrypted_output_hashes: [EncryptedOutputHash; 2],
    pub pricing: AssetOrchardPricingPublicFields,
    pub fee: u64,
}

impl AssetOrchardActionPublicFields {
    pub fn public_instance(
        &self,
    ) -> Result<[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN], AssetOrchardError> {
        let action_context = h_action(self)?;
        let eo0 = self.encrypted_output_hashes[0].as_fields();
        let eo1 = self.encrypted_output_hashes[1].as_fields();
        Ok([
            self.pool_domain,
            self.anchor,
            self.nullifiers[0],
            self.nullifiers[1],
            self.randomized_verification_keys[0].x,
            self.randomized_verification_keys[0].y,
            self.randomized_verification_keys[1].x,
            self.randomized_verification_keys[1].y,
            self.output_commitments[0],
            self.output_commitments[1],
            eo0[0],
            eo0[1],
            eo0[2],
            eo1[0],
            eo1[1],
            eo1[2],
            pallas::Base::from(self.fee),
            pallas::Base::from_u128(self.pricing.base_asset_tag.lo),
            pallas::Base::from_u128(self.pricing.base_asset_tag.hi),
            pallas::Base::from_u128(self.pricing.quote_asset_tag.lo),
            pallas::Base::from_u128(self.pricing.quote_asset_tag.hi),
            pallas::Base::from(self.pricing.ratio_numerator),
            pallas::Base::from(self.pricing.ratio_denominator),
            self.pricing.commitment[0],
            self.pricing.commitment[1],
            self.pricing.commitment[2],
            action_context[0],
            action_context[1],
        ])
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AssetOrchardPrivateEgressPublicFields {
    pub pool_domain: pallas::Base,
    pub anchor: pallas::Base,
    pub nullifier: pallas::Base,
    pub randomized_verification_key: RandomizedVerificationKeyFields,
    pub asset_tag: AssetTag,
    pub amount: u64,
    pub fee: u64,
    pub exit_binding_hash: [u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
}

impl AssetOrchardPrivateEgressPublicFields {
    pub fn public_instance(
        &self,
    ) -> Result<[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN], AssetOrchardError>
    {
        let action_context = private_egress_h_action(self)?;
        let exit_binding = binding_hash_fields(&self.exit_binding_hash)?;
        Ok([
            self.pool_domain,
            self.anchor,
            self.nullifier,
            self.randomized_verification_key.x,
            self.randomized_verification_key.y,
            pallas::Base::from_u128(self.asset_tag.lo),
            pallas::Base::from_u128(self.asset_tag.hi),
            pallas::Base::from(self.amount),
            pallas::Base::from(self.fee),
            exit_binding[0],
            exit_binding[1],
            action_context[0],
            action_context[1],
        ])
    }
}

pub fn h_action(
    fields: &AssetOrchardActionPublicFields,
) -> Result<[pallas::Base; 2], AssetOrchardError> {
    if fields.fee != 0 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_fee",
            "asset-orchard swap v1 requires fee 0",
        ));
    }
    let action_fields = h_action_fields(fields)?;
    poseidon_hash2(H_ACTION_HASH_NAME, &action_fields)
}

pub fn h_action_fields(
    fields: &AssetOrchardActionPublicFields,
) -> Result<[pallas::Base; ASSET_ORCHARD_H_ACTION_FIELD_COUNT], AssetOrchardError> {
    let eo0 = fields.encrypted_output_hashes[0].as_fields();
    let eo1 = fields.encrypted_output_hashes[1].as_fields();
    Ok([
        const_field(&format!("proof_system:{ASSET_ORCHARD_PROOF_SYSTEM_ID_V1}"))?,
        const_field(&format!("circuit:{ASSET_ORCHARD_SWAP_PROOF_BINDING_ID}"))?,
        const_field(&format!("schema:{ASSET_ORCHARD_ACTION_SCHEMA_V1}"))?,
        const_field(&format!("pool:{ASSET_ORCHARD_POOL_ID_V1}"))?,
        const_field(&format!("note_version:{ASSET_ORCHARD_NOTE_VERSION_V1}"))?,
        fields.pool_domain,
        fields.anchor,
        fields.nullifiers[0],
        fields.nullifiers[1],
        fields.randomized_verification_keys[0].x,
        fields.randomized_verification_keys[0].y,
        fields.randomized_verification_keys[1].x,
        fields.randomized_verification_keys[1].y,
        fields.output_commitments[0],
        fields.output_commitments[1],
        eo0[0],
        eo0[1],
        eo0[2],
        eo1[0],
        eo1[1],
        eo1[2],
        pallas::Base::from(fields.fee),
        pallas::Base::from_u128(fields.pricing.base_asset_tag.lo),
        pallas::Base::from_u128(fields.pricing.base_asset_tag.hi),
        pallas::Base::from_u128(fields.pricing.quote_asset_tag.lo),
        pallas::Base::from_u128(fields.pricing.quote_asset_tag.hi),
        pallas::Base::from(fields.pricing.ratio_numerator),
        pallas::Base::from(fields.pricing.ratio_denominator),
        fields.pricing.commitment[0],
        fields.pricing.commitment[1],
        fields.pricing.commitment[2],
        const_field("pricing_claim_version:1")?,
    ])
}

pub fn h_action_poseidon_inputs(
    fields: &AssetOrchardActionPublicFields,
) -> Result<[pallas::Base; ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT], AssetOrchardError> {
    let action_fields = h_action_fields(fields)?;
    h_action_poseidon_inputs_from_fields(&action_fields)
}

pub fn h_action_poseidon_inputs_from_fields(
    action_fields: &[pallas::Base; ASSET_ORCHARD_H_ACTION_FIELD_COUNT],
) -> Result<[pallas::Base; ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT], AssetOrchardError> {
    let mut input_fields = [pallas::Base::ZERO; ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT];
    input_fields[0] = const_field(H_ACTION_HASH_NAME)?;
    input_fields[1] = pallas::Base::from(ASSET_ORCHARD_H_ACTION_FIELD_COUNT as u64);
    input_fields[2..].copy_from_slice(action_fields);
    Ok(input_fields)
}

pub fn h_action_poseidon_inputs_from_public_instance(
    public_instance: &[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN],
) -> Result<[pallas::Base; ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT], AssetOrchardError> {
    let action_fields = [
        const_field(&format!("proof_system:{ASSET_ORCHARD_PROOF_SYSTEM_ID_V1}"))?,
        const_field(&format!("circuit:{ASSET_ORCHARD_SWAP_PROOF_BINDING_ID}"))?,
        const_field(&format!("schema:{ASSET_ORCHARD_ACTION_SCHEMA_V1}"))?,
        const_field(&format!("pool:{ASSET_ORCHARD_POOL_ID_V1}"))?,
        const_field(&format!("note_version:{ASSET_ORCHARD_NOTE_VERSION_V1}"))?,
        public_instance[0],
        public_instance[1],
        public_instance[2],
        public_instance[3],
        public_instance[4],
        public_instance[5],
        public_instance[6],
        public_instance[7],
        public_instance[8],
        public_instance[9],
        public_instance[10],
        public_instance[11],
        public_instance[12],
        public_instance[13],
        public_instance[14],
        public_instance[15],
        public_instance[16],
        public_instance[17],
        public_instance[18],
        public_instance[19],
        public_instance[20],
        public_instance[21],
        public_instance[22],
        public_instance[23],
        public_instance[24],
        public_instance[25],
        const_field("pricing_claim_version:1")?,
    ];
    h_action_poseidon_inputs_from_fields(&action_fields)
}

pub fn swap_binding_hash(
    fields: &AssetOrchardActionPublicFields,
) -> Result<[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES], AssetOrchardError> {
    let action_context = h_action(fields)?;
    let mut out = [0u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES];
    out[0..32].copy_from_slice(&field_enc(action_context[0]));
    out[32..64].copy_from_slice(&field_enc(action_context[1]));
    Ok(out)
}

pub fn private_egress_h_action(
    fields: &AssetOrchardPrivateEgressPublicFields,
) -> Result<[pallas::Base; 2], AssetOrchardError> {
    if fields.amount == 0 {
        return Err(AssetOrchardError::new(
            "zero_private_egress_amount",
            "asset-orchard private egress amount must be nonzero",
        ));
    }
    if fields.fee != 0 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_private_egress_fee",
            "asset-orchard private egress v1 requires fee 0",
        ));
    }
    fields.asset_tag.validate()?;
    let action_fields = private_egress_h_action_fields(fields)?;
    poseidon_hash2(PRIVATE_EGRESS_H_ACTION_HASH_NAME, &action_fields)
}

pub fn private_egress_h_action_fields(
    fields: &AssetOrchardPrivateEgressPublicFields,
) -> Result<[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_FIELD_COUNT], AssetOrchardError> {
    let exit_binding = binding_hash_fields(&fields.exit_binding_hash)?;
    Ok([
        const_field(&format!("proof_system:{ASSET_ORCHARD_PROOF_SYSTEM_ID_V1}"))?,
        const_field(&format!(
            "circuit:{ASSET_ORCHARD_PRIVATE_EGRESS_PROOF_BINDING_ID}"
        ))?,
        const_field(&format!(
            "schema:{ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1}"
        ))?,
        const_field(&format!("pool:{ASSET_ORCHARD_POOL_ID_V1}"))?,
        const_field(&format!("note_version:{ASSET_ORCHARD_NOTE_VERSION_V1}"))?,
        fields.pool_domain,
        fields.anchor,
        fields.nullifier,
        fields.randomized_verification_key.x,
        fields.randomized_verification_key.y,
        pallas::Base::from_u128(fields.asset_tag.lo),
        pallas::Base::from_u128(fields.asset_tag.hi),
        pallas::Base::from(fields.amount),
        pallas::Base::from(fields.fee),
        exit_binding[0],
        exit_binding[1],
    ])
}

pub fn private_egress_h_action_poseidon_inputs(
    fields: &AssetOrchardPrivateEgressPublicFields,
) -> Result<
    [pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT],
    AssetOrchardError,
> {
    let action_fields = private_egress_h_action_fields(fields)?;
    private_egress_h_action_poseidon_inputs_from_fields(&action_fields)
}

pub fn private_egress_h_action_poseidon_inputs_from_fields(
    action_fields: &[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_FIELD_COUNT],
) -> Result<
    [pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT],
    AssetOrchardError,
> {
    let mut input_fields =
        [pallas::Base::ZERO; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT];
    input_fields[0] = const_field(PRIVATE_EGRESS_H_ACTION_HASH_NAME)?;
    input_fields[1] = pallas::Base::from(ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_FIELD_COUNT as u64);
    input_fields[2..].copy_from_slice(action_fields);
    Ok(input_fields)
}

pub fn private_egress_h_action_poseidon_inputs_from_public_instance(
    public_instance: &[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN],
) -> Result<
    [pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT],
    AssetOrchardError,
> {
    let action_fields = [
        const_field(&format!("proof_system:{ASSET_ORCHARD_PROOF_SYSTEM_ID_V1}"))?,
        const_field(&format!(
            "circuit:{ASSET_ORCHARD_PRIVATE_EGRESS_PROOF_BINDING_ID}"
        ))?,
        const_field(&format!(
            "schema:{ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1}"
        ))?,
        const_field(&format!("pool:{ASSET_ORCHARD_POOL_ID_V1}"))?,
        const_field(&format!("note_version:{ASSET_ORCHARD_NOTE_VERSION_V1}"))?,
        public_instance[0],
        public_instance[1],
        public_instance[2],
        public_instance[3],
        public_instance[4],
        public_instance[5],
        public_instance[6],
        public_instance[7],
        public_instance[8],
        public_instance[9],
        public_instance[10],
    ];
    private_egress_h_action_poseidon_inputs_from_fields(&action_fields)
}

pub fn private_egress_action_binding_hash(
    fields: &AssetOrchardPrivateEgressPublicFields,
) -> Result<[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES], AssetOrchardError> {
    let action_context = private_egress_h_action(fields)?;
    let mut out = [0u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES];
    out[0..32].copy_from_slice(&field_enc(action_context[0]));
    out[32..64].copy_from_slice(&field_enc(action_context[1]));
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct AssetOrchardPrivateEgressExitBindingPreimage<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: [u8; 32],
    pub protocol_version: u32,
    pub pool_id: &'a str,
    pub circuit_id: &'a str,
    pub pool_domain: pallas::Base,
    pub to: &'a str,
    pub asset_id: &'a str,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: &'a str,
    pub disclosure_hash: &'a str,
}

pub fn asset_orchard_private_egress_exit_binding_hash(
    preimage: &AssetOrchardPrivateEgressExitBindingPreimage<'_>,
) -> Result<[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES], AssetOrchardError> {
    validate_private_egress_exit_fields(
        preimage.chain_id,
        preimage.pool_id,
        preimage.protocol_version,
        preimage.to,
        preimage.asset_id,
        preimage.amount,
        preimage.fee,
        preimage.policy_id,
        preimage.disclosure_hash,
    )?;
    let fields = [
        const_field(&format!("proof_system:{ASSET_ORCHARD_PROOF_SYSTEM_ID_V1}"))?,
        const_field(&format!(
            "circuit:{}",
            supported_asset_orchard_private_egress_circuit_id(preimage.circuit_id)?
        ))?,
        const_field(&format!(
            "schema:{ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1}"
        ))?,
        const_field(&format!("pool:{}", preimage.pool_id))?,
        const_field(&format!("note_version:{ASSET_ORCHARD_NOTE_VERSION_V1}"))?,
        text_to_field("private_egress.chain_id", preimage.chain_id)?,
        bytes_to_field("private_egress.genesis_hash", &preimage.genesis_hash)?,
        pallas::Base::from(u64::from(preimage.protocol_version)),
        preimage.pool_domain,
        text_to_field("private_egress.to", preimage.to)?,
        text_to_field("private_egress.asset_id", preimage.asset_id)?,
        pallas::Base::from(preimage.amount),
        pallas::Base::from(preimage.fee),
        text_to_field("private_egress.policy_id", preimage.policy_id)?,
        text_to_field("private_egress.disclosure_hash", preimage.disclosure_hash)?,
    ];
    let hash = poseidon_hash2(
        std::str::from_utf8(PRIVATE_EGRESS_EXIT_BINDING_DOMAIN).map_err(|_| {
            AssetOrchardError::new(
                "invalid_private_egress_exit_binding_domain",
                "private egress exit-binding domain is not valid UTF-8",
            )
        })?,
        &fields,
    )?;
    let mut out = [0u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES];
    out[0..32].copy_from_slice(&field_enc(hash[0]));
    out[32..64].copy_from_slice(&field_enc(hash[1]));
    Ok(out)
}

fn binding_hash_fields(
    hash: &[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
) -> Result<[pallas::Base; 2], AssetOrchardError> {
    let first: [u8; ASSET_ORCHARD_FIELD_BYTES] = hash[0..32].try_into().map_err(|_| {
        AssetOrchardError::new("digest_slice", "invalid private egress binding limb 0")
    })?;
    let second: [u8; ASSET_ORCHARD_FIELD_BYTES] = hash[32..64].try_into().map_err(|_| {
        AssetOrchardError::new("digest_slice", "invalid private egress binding limb 1")
    })?;
    let first = Option::<pallas::Base>::from(pallas::Base::from_repr(first)).ok_or_else(|| {
        AssetOrchardError::new(
            "noncanonical_private_egress_exit_binding_hash",
            "private egress exit_binding_hash limb 0 is not a canonical Pallas-base field",
        )
    })?;
    let second =
        Option::<pallas::Base>::from(pallas::Base::from_repr(second)).ok_or_else(|| {
            AssetOrchardError::new(
                "noncanonical_private_egress_exit_binding_hash",
                "private egress exit_binding_hash limb 1 is not a canonical Pallas-base field",
            )
        })?;
    Ok([first, second])
}

#[derive(Debug, Clone)]
pub struct AssetOrchardSigPreimage<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: [u8; 32],
    pub protocol_version: u32,
    pub pool_id: &'a str,
    pub circuit_id: &'a str,
    pub pool_domain: pallas::Base,
    pub anchor: pallas::Base,
    pub nullifiers: [pallas::Base; 2],
    pub randomized_verification_keys: [pallas::Affine; 2],
    pub output_commitments: [pallas::Base; 2],
    pub encrypted_outputs: [&'a [u8]; 2],
    pub accounting_inputs: [&'a AssetOrchardSwapAccountingRecord; 2],
    pub accounting_outputs: [&'a AssetOrchardSwapAccountingRecord; 2],
    pub swap_binding_hash: [u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
    pub fee: u64,
}

pub fn h_sig(
    preimage: &AssetOrchardSigPreimage<'_>,
) -> Result<[u8; ASSET_ORCHARD_SIGHASH_BYTES], AssetOrchardError> {
    validate_canonical_text("chain_id", preimage.chain_id, 256)?;
    validate_canonical_text("pool_id", preimage.pool_id, ASSET_ORCHARD_MAX_POOL_ID_BYTES)?;
    if preimage.protocol_version == 0 {
        return Err(AssetOrchardError::new(
            "invalid_protocol_version",
            "protocol_version must be nonzero",
        ));
    }
    if preimage.fee != 0 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_fee",
            "asset-orchard swap v1 requires fee 0",
        ));
    }
    let mut payload = Vec::new();
    payload.extend_from_slice(H_SIG_DOMAIN);
    payload.extend_from_slice(&1u16.to_le_bytes());
    append_len_bytes_vec(&mut payload, preimage.chain_id.as_bytes())?;
    payload.extend_from_slice(&preimage.genesis_hash);
    payload.extend_from_slice(&preimage.protocol_version.to_le_bytes());
    append_len_bytes_vec(&mut payload, preimage.pool_id.as_bytes())?;
    append_len_bytes_vec(&mut payload, ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.as_bytes())?;
    append_len_bytes_vec(
        &mut payload,
        supported_asset_orchard_swap_circuit_id(preimage.circuit_id)?.as_bytes(),
    )?;
    append_len_bytes_vec(&mut payload, ASSET_ORCHARD_ACTION_SCHEMA_V1.as_bytes())?;
    payload.extend_from_slice(&field_enc(preimage.pool_domain));
    payload.extend_from_slice(&field_enc(preimage.anchor));
    payload.extend_from_slice(&field_enc(preimage.nullifiers[0]));
    payload.extend_from_slice(&field_enc(preimage.nullifiers[1]));
    payload.extend_from_slice(&point_enc(preimage.randomized_verification_keys[0])?);
    payload.extend_from_slice(&point_enc(preimage.randomized_verification_keys[1])?);
    payload.extend_from_slice(&field_enc(preimage.output_commitments[0]));
    payload.extend_from_slice(&field_enc(preimage.output_commitments[1]));
    append_len_bytes_vec(&mut payload, preimage.encrypted_outputs[0])?;
    append_len_bytes_vec(&mut payload, preimage.encrypted_outputs[1])?;
    for record in preimage.accounting_inputs {
        append_asset_orchard_accounting_record(&mut payload, record)?;
    }
    for record in preimage.accounting_outputs {
        append_asset_orchard_accounting_record(&mut payload, record)?;
    }
    payload.extend_from_slice(&preimage.swap_binding_hash);
    payload.extend_from_slice(&preimage.fee.to_le_bytes());
    let digest = Sha3_256::digest(&payload);
    digest.as_slice().try_into().map_err(|_| {
        AssetOrchardError::new(
            "digest_slice",
            "invalid asset-orchard sighash digest length",
        )
    })
}

fn append_asset_orchard_accounting_record(
    payload: &mut Vec<u8>,
    record: &AssetOrchardSwapAccountingRecord,
) -> Result<(), AssetOrchardError> {
    fixed_lower_hex_array::<ASSET_ORCHARD_FIELD_BYTES>(
        "asset_orchard_accounting_output_commitment",
        &record.output_commitment,
    )?;
    record.value_commitment.to_affine()?;
    append_len_bytes_vec(payload, record.output_commitment.as_bytes())?;
    append_len_bytes_vec(payload, record.value_commitment.as_hex().as_bytes())?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AssetOrchardPrivateEgressPreimage<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: [u8; 32],
    pub protocol_version: u32,
    pub pool_id: &'a str,
    pub circuit_id: &'a str,
    pub pool_domain: pallas::Base,
    pub anchor: pallas::Base,
    pub to: &'a str,
    pub asset_id: &'a str,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: &'a str,
    pub disclosure_hash: &'a str,
    pub nullifier: pallas::Base,
    pub randomized_verification_key: pallas::Affine,
    pub asset_tag_lo: u128,
    pub asset_tag_hi: u128,
    pub exit_binding_hash: [u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
}

pub fn asset_orchard_private_egress_sighash(
    preimage: &AssetOrchardPrivateEgressPreimage<'_>,
) -> Result<[u8; ASSET_ORCHARD_SIGHASH_BYTES], AssetOrchardError> {
    validate_private_egress_exit_fields(
        preimage.chain_id,
        preimage.pool_id,
        preimage.protocol_version,
        preimage.to,
        preimage.asset_id,
        preimage.amount,
        preimage.fee,
        preimage.policy_id,
        preimage.disclosure_hash,
    )?;
    AssetTag {
        lo: preimage.asset_tag_lo,
        hi: preimage.asset_tag_hi,
    }
    .validate()?;
    binding_hash_fields(&preimage.exit_binding_hash)?;

    let mut payload = Vec::new();
    payload.extend_from_slice(PRIVATE_EGRESS_H_SIG_DOMAIN);
    payload.extend_from_slice(&1u16.to_le_bytes());
    append_len_bytes_vec(
        &mut payload,
        ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1.as_bytes(),
    )?;
    append_len_bytes_vec(&mut payload, preimage.chain_id.as_bytes())?;
    payload.extend_from_slice(&preimage.genesis_hash);
    payload.extend_from_slice(&preimage.protocol_version.to_le_bytes());
    append_len_bytes_vec(&mut payload, preimage.pool_id.as_bytes())?;
    append_len_bytes_vec(&mut payload, ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.as_bytes())?;
    append_len_bytes_vec(
        &mut payload,
        supported_asset_orchard_private_egress_circuit_id(preimage.circuit_id)?.as_bytes(),
    )?;
    payload.extend_from_slice(&field_enc(preimage.pool_domain));
    payload.extend_from_slice(&field_enc(preimage.anchor));
    append_len_bytes_vec(&mut payload, preimage.to.as_bytes())?;
    append_len_bytes_vec(&mut payload, preimage.asset_id.as_bytes())?;
    payload.extend_from_slice(&preimage.amount.to_le_bytes());
    payload.extend_from_slice(&preimage.fee.to_le_bytes());
    append_len_bytes_vec(&mut payload, preimage.policy_id.as_bytes())?;
    append_len_bytes_vec(&mut payload, preimage.disclosure_hash.as_bytes())?;
    payload.extend_from_slice(&field_enc(preimage.nullifier));
    payload.extend_from_slice(&point_enc(preimage.randomized_verification_key)?);
    payload.extend_from_slice(&preimage.asset_tag_lo.to_le_bytes());
    payload.extend_from_slice(&preimage.asset_tag_hi.to_le_bytes());
    payload.extend_from_slice(&preimage.exit_binding_hash);
    let digest = Sha3_256::digest(&payload);
    digest.as_slice().try_into().map_err(|_| {
        AssetOrchardError::new(
            "digest_slice",
            "invalid asset-orchard private-egress sighash digest length",
        )
    })
}

#[derive(Debug, Clone)]
pub struct AssetOrchardDisclosedEgressPreimage<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: [u8; 32],
    pub protocol_version: u32,
    pub pool_id: &'a str,
    pub pool_domain: pallas::Base,
    pub to: &'a str,
    pub asset_id: &'a str,
    pub amount: u64,
    pub output_commitment: pallas::Base,
    pub nullifier: pallas::Base,
    pub spend_auth_verification_key: pallas::Affine,
    pub spend_auth_randomizer: pallas::Scalar,
    pub randomized_verification_key: pallas::Affine,
}

#[derive(Debug, Clone)]
pub struct AssetOrchardDisclosedEgressCheck<'a> {
    pub preimage: AssetOrchardDisclosedEgressPreimage<'a>,
    pub note: &'a AssetOrchardPublicNoteOpening,
    pub nk: &'a AssetOrchardFieldElement,
    pub rivk: &'a str,
    pub spend_authorization_signature: &'a AssetOrchardSpendAuthSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardDisclosedEgressAuthorization {
    pub nullifier: AssetOrchardFieldElement,
    pub spend_auth_verification_key: AssetOrchardPoint,
    pub spend_auth_randomizer: String,
    pub randomized_verification_key: AssetOrchardPoint,
    pub spend_authorization_signature: AssetOrchardSpendAuthSignature,
    pub sighash: String,
}

pub fn validate_asset_orchard_wallet_note_for_pool(
    note: &AssetOrchardWalletNote,
    pool_domain: pallas::Base,
) -> Result<(), AssetOrchardError> {
    if note.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_wallet_note_pool",
            format!(
                "unsupported asset-orchard wallet note pool `{}`",
                note.pool_id
            ),
        ));
    }
    if note.pool_domain.to_field()? != pool_domain {
        return Err(AssetOrchardError::new(
            "asset_orchard_wallet_note_pool_domain_mismatch",
            "asset-orchard wallet note pool domain does not match chain/genesis/protocol",
        ));
    }
    note.note.validate_for_asset(&note.asset_id, note.value)?;
    let expected_cmx = note.note.cmx(pool_domain)?;
    if expected_cmx != note.output_commitment {
        return Err(AssetOrchardError::new(
            "asset_orchard_wallet_note_cmx_mismatch",
            "asset-orchard wallet note commitment does not match note opening",
        ));
    }
    Ok(())
}

pub fn asset_orchard_wallet_note_nullifier(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    note: &AssetOrchardWalletNote,
) -> Result<AssetOrchardFieldElement, AssetOrchardError> {
    let pool_domain =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    validate_asset_orchard_wallet_note_for_pool(note, pool_domain)?;
    let opening = note.note.to_note_opening()?;
    let cmx = opening.cmx(pool_domain)?;
    Ok(AssetOrchardFieldElement::from_field(
        asset_derive_nullifier(
            pool_domain,
            note.nk.to_field()?,
            opening.rho,
            opening.psi,
            cmx,
        )?,
    ))
}

pub fn asset_orchard_egress_randomizer(
    _output_commitment: &AssetOrchardFieldElement,
    to: &str,
    asset_id: &str,
    _amount: u64,
) -> Result<String, AssetOrchardError> {
    validate_canonical_text("asset_orchard_egress_to", to, 256)?;
    validate_canonical_text(
        "asset_orchard_egress_asset_id",
        asset_id,
        ASSET_ORCHARD_MAX_ASSET_ID_BYTES,
    )?;
    Ok(bytes_to_hex(&scalar_enc(random_pallas_scalar_nonzero())))
}

pub fn asset_orchard_disclosed_egress_sighash(
    preimage: &AssetOrchardDisclosedEgressPreimage<'_>,
) -> Result<[u8; ASSET_ORCHARD_SIGHASH_BYTES], AssetOrchardError> {
    validate_canonical_text("chain_id", preimage.chain_id, 256)?;
    validate_canonical_text("pool_id", preimage.pool_id, ASSET_ORCHARD_MAX_POOL_ID_BYTES)?;
    validate_canonical_text("asset_orchard_egress_to", preimage.to, 256)?;
    validate_canonical_text(
        "asset_orchard_egress_asset_id",
        preimage.asset_id,
        ASSET_ORCHARD_MAX_ASSET_ID_BYTES,
    )?;
    if preimage.protocol_version == 0 {
        return Err(AssetOrchardError::new(
            "invalid_protocol_version",
            "protocol_version must be nonzero",
        ));
    }
    if preimage.amount == 0 {
        return Err(AssetOrchardError::new(
            "zero_egress_amount",
            "asset-orchard disclosed egress amount must be nonzero",
        ));
    }
    if preimage.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_pool",
            format!("unsupported asset-orchard pool `{}`", preimage.pool_id),
        ));
    }

    let mut payload = Vec::new();
    payload.extend_from_slice(EGRESS_H_SIG_DOMAIN);
    payload.extend_from_slice(&1u16.to_le_bytes());
    append_len_bytes_vec(
        &mut payload,
        ASSET_ORCHARD_DISCLOSED_EGRESS_SCHEMA_V1.as_bytes(),
    )?;
    append_len_bytes_vec(&mut payload, preimage.chain_id.as_bytes())?;
    payload.extend_from_slice(&preimage.genesis_hash);
    payload.extend_from_slice(&preimage.protocol_version.to_le_bytes());
    append_len_bytes_vec(&mut payload, preimage.pool_id.as_bytes())?;
    payload.extend_from_slice(&field_enc(preimage.pool_domain));
    append_len_bytes_vec(&mut payload, preimage.to.as_bytes())?;
    append_len_bytes_vec(&mut payload, preimage.asset_id.as_bytes())?;
    payload.extend_from_slice(&preimage.amount.to_le_bytes());
    payload.extend_from_slice(&field_enc(preimage.output_commitment));
    payload.extend_from_slice(&field_enc(preimage.nullifier));
    payload.extend_from_slice(&point_enc(preimage.spend_auth_verification_key)?);
    payload.extend_from_slice(&scalar_enc(preimage.spend_auth_randomizer));
    payload.extend_from_slice(&point_enc(preimage.randomized_verification_key)?);
    let digest = Sha3_256::digest(&payload);
    digest.as_slice().try_into().map_err(|_| {
        AssetOrchardError::new(
            "digest_slice",
            "invalid asset-orchard disclosed-egress sighash digest length",
        )
    })
}

pub fn build_asset_orchard_disclosed_egress_authorization(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    to: &str,
    note: &AssetOrchardWalletNote,
) -> Result<AssetOrchardDisclosedEgressAuthorization, AssetOrchardError> {
    let pool_domain =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    validate_asset_orchard_wallet_note_for_pool(note, pool_domain)?;
    let nullifier =
        asset_orchard_wallet_note_nullifier(chain_id, genesis_hash, protocol_version, note)?;
    let signing_key =
        asset_orchard_spend_signing_key_from_hex(note.spend_auth_signing_key.as_str())?;
    let verification_key = VerificationKey::from(&signing_key);
    let ak = verification_key_affine(&verification_key)?;
    let alpha_hex =
        asset_orchard_egress_randomizer(&note.output_commitment, to, &note.asset_id, note.value)?;
    let alpha = parse_pallas_scalar(&alpha_hex)?;
    let rk = randomized_spend_auth_key(ak, alpha)?;
    let preimage = AssetOrchardDisclosedEgressPreimage {
        chain_id,
        genesis_hash,
        protocol_version,
        pool_id: &note.pool_id,
        pool_domain,
        to,
        asset_id: &note.asset_id,
        amount: note.value,
        output_commitment: note.output_commitment.to_field()?,
        nullifier: nullifier.to_field()?,
        spend_auth_verification_key: ak,
        spend_auth_randomizer: alpha,
        randomized_verification_key: rk,
    };
    let sighash = asset_orchard_disclosed_egress_sighash(&preimage)?;
    let signature = AssetOrchardSpendAuthSignature::from_orchard(
        &signing_key.randomize(&alpha).sign(OsRng, &sighash),
    );
    let authorization = AssetOrchardDisclosedEgressAuthorization {
        nullifier,
        spend_auth_verification_key: AssetOrchardPoint::from_affine(ak)?,
        spend_auth_randomizer: alpha_hex,
        randomized_verification_key: AssetOrchardPoint::from_affine(rk)?,
        spend_authorization_signature: signature,
        sighash: bytes_to_hex(&sighash),
    };
    verify_asset_orchard_disclosed_egress(&AssetOrchardDisclosedEgressCheck {
        preimage,
        note: &note.note,
        nk: &note.nk,
        rivk: note.rivk.as_str(),
        spend_authorization_signature: &authorization.spend_authorization_signature,
    })?;
    Ok(authorization)
}

pub fn verify_asset_orchard_disclosed_egress(
    check: &AssetOrchardDisclosedEgressCheck<'_>,
) -> Result<(), AssetOrchardError> {
    if check.preimage.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_pool",
            format!(
                "unsupported asset-orchard pool `{}`",
                check.preimage.pool_id
            ),
        ));
    }
    check
        .note
        .validate_for_asset(check.preimage.asset_id, check.preimage.amount)?;
    let expected_cmx = check.note.cmx(check.preimage.pool_domain)?;
    if expected_cmx.to_field()? != check.preimage.output_commitment {
        return Err(AssetOrchardError::new(
            "asset_orchard_egress_cmx_mismatch",
            "asset-orchard disclosed egress commitment does not match note opening",
        ));
    }
    let note_opening = check.note.to_note_opening()?;
    let expected_nullifier = asset_derive_nullifier(
        check.preimage.pool_domain,
        check.nk.to_field()?,
        note_opening.rho,
        note_opening.psi,
        check.preimage.output_commitment,
    )?;
    if expected_nullifier != check.preimage.nullifier {
        return Err(AssetOrchardError::new(
            "asset_orchard_egress_nullifier_mismatch",
            "asset-orchard disclosed egress nullifier does not match note opening and nk",
        ));
    }

    let rivk = parse_pallas_scalar(check.rivk)?;
    let ivk = orchard_commit_ivk(
        check.preimage.spend_auth_verification_key,
        check.nk.to_field()?,
        rivk,
    )?;
    let ivk_scalar = Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(ivk.to_repr()))
        .ok_or_else(|| {
            AssetOrchardError::new(
                "invalid_asset_orchard_ivk_scalar",
                "derived Orchard ivk cannot be represented as a Pallas scalar",
            )
        })?;
    let expected_pk_d = (pallas::Point::from(note_opening.g_d) * ivk_scalar).to_affine();
    if point_enc(expected_pk_d)? != point_enc(note_opening.pk_d)? {
        return Err(AssetOrchardError::new(
            "asset_orchard_egress_spend_authority_mismatch",
            "asset-orchard disclosed egress spend authority does not control the note",
        ));
    }

    let expected_rk = randomized_spend_auth_key(
        check.preimage.spend_auth_verification_key,
        check.preimage.spend_auth_randomizer,
    )?;
    if point_enc(expected_rk)? != point_enc(check.preimage.randomized_verification_key)? {
        return Err(AssetOrchardError::new(
            "asset_orchard_egress_randomized_key_mismatch",
            "asset-orchard disclosed egress randomized verification key is not derived from ak and alpha",
        ));
    }
    let sighash = asset_orchard_disclosed_egress_sighash(&check.preimage)?;
    let rk = VerificationKey::<SpendAuth>::try_from(point_enc(
        check.preimage.randomized_verification_key,
    )?)
    .map_err(|_| {
        AssetOrchardError::new(
            "invalid_asset_orchard_randomized_verification_key",
            "asset-orchard disclosed egress randomized verification key is invalid",
        )
    })?;
    rk.verify(&sighash, &check.spend_authorization_signature.to_orchard()?)
        .map_err(|_| {
            AssetOrchardError::new(
                "asset_orchard_egress_spend_authorization_failed",
                "asset-orchard disclosed egress spend authorization signature failed",
            )
        })?;
    Ok(())
}

fn asset_orchard_spend_signing_key_from_hex(
    signing_key_hex: &str,
) -> Result<SigningKey<SpendAuth>, AssetOrchardError> {
    let bytes =
        fixed_lower_hex_array::<32>("asset_orchard_spend_auth_signing_key", signing_key_hex)?;
    SigningKey::<SpendAuth>::try_from(bytes).map_err(|_| {
        AssetOrchardError::new(
            "invalid_asset_orchard_spend_auth_signing_key",
            "asset-orchard wallet note spend authorization key is invalid",
        )
    })
}

fn randomized_spend_auth_key(
    ak: pallas::Affine,
    alpha: pallas::Scalar,
) -> Result<pallas::Affine, AssetOrchardError> {
    let rk = (pallas::Point::from(ak) + asset_spend_auth_g() * alpha).to_affine();
    reject_identity_point("asset_orchard_randomized_verification_key", rk)?;
    Ok(rk)
}

#[derive(Debug, Clone)]
pub struct AssetNoteOpening {
    pub diversifier: [u8; ASSET_ORCHARD_DIVERSIFIER_BYTES],
    pub g_d: pallas::Affine,
    pub pk_d: pallas::Affine,
    pub asset_tag: AssetTag,
    pub value: u64,
    pub rho: pallas::Base,
    pub psi: pallas::Base,
    pub rcm: pallas::Scalar,
}

impl AssetNoteOpening {
    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        if self.value == 0 {
            return Err(AssetOrchardError::new(
                "zero_note_value",
                "asset-orchard note value must be nonzero",
            ));
        }
        self.asset_tag.validate()?;
        reject_identity_point("g_d", self.g_d)?;
        reject_identity_point("pk_d", self.pk_d)?;
        if bool::from(self.rcm.is_zero()) {
            return Err(AssetOrchardError::new(
                "zero_rcm",
                "asset-orchard note rcm must be nonzero",
            ));
        }
        Ok(())
    }

    pub fn commitment_point(
        &self,
        pool_domain: pallas::Base,
    ) -> Result<pallas::Point, AssetOrchardError> {
        self.validate()?;
        let message = asset_note_message_bits(pool_domain, self)?;
        let domain = sinsemilla::CommitDomain::new(ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1);
        Option::<pallas::Point>::from(domain.commit(message.into_iter(), &self.rcm)).ok_or_else(
            || {
                AssetOrchardError::new(
                    "asset_note_commit_failed",
                    "asset note commitment produced an invalid Sinsemilla point",
                )
            },
        )
    }

    pub fn cmx(&self, pool_domain: pallas::Base) -> Result<pallas::Base, AssetOrchardError> {
        let point = self.commitment_point(pool_domain)?;
        extract_p(point)
    }
}

pub fn asset_note_message_bits(
    pool_domain: pallas::Base,
    note: &AssetNoteOpening,
) -> Result<Vec<bool>, AssetOrchardError> {
    note.validate()?;
    let mut bits = Vec::with_capacity(ASSET_ORCHARD_NOTE_MESSAGE_BITS);
    bits.extend(i2lebsp_from_field(pool_domain, 255));
    bits.extend(i2lebsp_from_u128(note.asset_tag.lo, 128));
    bits.extend(i2lebsp_from_u128(note.asset_tag.hi, 128));
    bits.extend(point_bits(note.g_d)?);
    bits.extend(point_bits(note.pk_d)?);
    bits.extend(i2lebsp_from_u64(note.value, 64));
    bits.extend(i2lebsp_from_field(note.rho, 255));
    bits.extend(i2lebsp_from_field(note.psi, 255));
    Ok(bits)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AssetNoteMessageSource {
    PoolDomain,
    AssetTagLo,
    AssetTagHi,
    GdX,
    GdYSign,
    PkdX,
    PkdYSign,
    Value,
    Rho,
    Psi,
    Padding,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AssetNoteMessageSegment {
    pub piece_index: usize,
    pub piece_bit_offset: usize,
    pub source: AssetNoteMessageSource,
    pub source_bit_offset: usize,
    pub bit_len: usize,
}

pub fn asset_note_message_segments() -> Vec<AssetNoteMessageSegment> {
    let sources = [
        (AssetNoteMessageSource::PoolDomain, 255usize),
        (AssetNoteMessageSource::AssetTagLo, 128),
        (AssetNoteMessageSource::AssetTagHi, 128),
        (AssetNoteMessageSource::GdX, 255),
        (AssetNoteMessageSource::GdYSign, 1),
        (AssetNoteMessageSource::PkdX, 255),
        (AssetNoteMessageSource::PkdYSign, 1),
        (AssetNoteMessageSource::Value, 64),
        (AssetNoteMessageSource::Rho, 255),
        (AssetNoteMessageSource::Psi, 255),
        (AssetNoteMessageSource::Padding, 3),
    ];

    let mut segments = Vec::new();
    let mut piece_index = 0usize;
    let mut piece_bit_offset = 0usize;
    for (source, source_bits) in sources {
        let mut source_bit_offset = 0usize;
        while source_bit_offset < source_bits {
            let remaining_source = source_bits - source_bit_offset;
            let remaining_piece = ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS - piece_bit_offset;
            let bit_len = remaining_source.min(remaining_piece);
            segments.push(AssetNoteMessageSegment {
                piece_index,
                piece_bit_offset,
                source,
                source_bit_offset,
                bit_len,
            });
            source_bit_offset += bit_len;
            piece_bit_offset += bit_len;
            if piece_bit_offset == ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS {
                piece_index += 1;
                piece_bit_offset = 0;
            }
        }
    }

    debug_assert_eq!(
        piece_index * ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS + piece_bit_offset,
        ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS
    );
    segments
}

pub fn poseidon_hash1(
    name: &str,
    fields: &[pallas::Base],
) -> Result<pallas::Base, AssetOrchardError> {
    Ok(poseidon_hash2(name, fields)?[0])
}

pub fn poseidon_hash2(
    name: &str,
    fields: &[pallas::Base],
) -> Result<[pallas::Base; 2], AssetOrchardError> {
    validate_canonical_text("poseidon_hash_name", name, 256)?;
    let len = u64::try_from(fields.len()).map_err(|_| {
        AssetOrchardError::new(
            "poseidon_input_too_large",
            "field input length overflows u64",
        )
    })?;
    let mut input_fields = Vec::with_capacity(fields.len() + 2);
    input_fields.push(const_field(name)?);
    input_fields.push(pallas::Base::from(len));
    input_fields.extend_from_slice(fields);
    poseidon_pallas_sponge_squeeze_2(&input_fields)
}

pub fn field_enc(field: pallas::Base) -> [u8; ASSET_ORCHARD_FIELD_BYTES] {
    field.to_repr()
}

pub fn scalar_enc(scalar: pallas::Scalar) -> [u8; ASSET_ORCHARD_FIELD_BYTES] {
    scalar.to_repr()
}

pub fn asset_orchard_scalar_from_hex(value: &str) -> Result<pallas::Scalar, AssetOrchardError> {
    parse_pallas_scalar(value)
}

pub fn point_enc(
    point: pallas::Affine,
) -> Result<[u8; ASSET_ORCHARD_POINT_BYTES], AssetOrchardError> {
    reject_identity_point("point", point)?;
    Ok(point.to_bytes())
}

fn poseidon_pallas_sponge_squeeze_2(
    input_fields: &[pallas::Base],
) -> Result<[pallas::Base; 2], AssetOrchardError> {
    let (round_constants, mds_matrix, _) = P128Pow5T3::constants();
    let mut state = [pallas::Base::ZERO; ASSET_ORCHARD_POSEIDON_WIDTH];
    let mut offset = 0usize;
    while offset < input_fields.len() {
        for lane in 0..ASSET_ORCHARD_POSEIDON_RATE {
            let value = input_fields
                .get(offset + lane)
                .copied()
                .unwrap_or(pallas::Base::ZERO);
            state[lane] += value;
        }
        poseidon_permute::<
            pallas::Base,
            P128Pow5T3,
            ASSET_ORCHARD_POSEIDON_WIDTH,
            ASSET_ORCHARD_POSEIDON_RATE,
        >(&mut state, &mds_matrix, &round_constants);
        offset += ASSET_ORCHARD_POSEIDON_RATE;
    }
    if input_fields.is_empty() {
        poseidon_permute::<
            pallas::Base,
            P128Pow5T3,
            ASSET_ORCHARD_POSEIDON_WIDTH,
            ASSET_ORCHARD_POSEIDON_RATE,
        >(&mut state, &mds_matrix, &round_constants);
    }
    Ok([state[0], state[1]])
}

fn poseidon_permute<F: Field, S: Spec<F, T, RATE>, const T: usize, const RATE: usize>(
    state: &mut State<F, T>,
    mds: &Mds<F, T>,
    round_constants: &[[F; T]],
) {
    let r_f = S::full_rounds() / 2;
    let r_p = S::partial_rounds();

    for rc in round_constants.iter().take(r_f) {
        poseidon_full_round::<F, S, T, RATE>(state, mds, rc);
    }
    for rc in round_constants.iter().skip(r_f).take(r_p) {
        poseidon_partial_round::<F, S, T, RATE>(state, mds, rc);
    }
    for rc in round_constants.iter().skip(r_f + r_p).take(r_f) {
        poseidon_full_round::<F, S, T, RATE>(state, mds, rc);
    }
}

fn poseidon_full_round<F: Field, S: Spec<F, T, RATE>, const T: usize, const RATE: usize>(
    state: &mut State<F, T>,
    mds: &Mds<F, T>,
    rc: &[F; T],
) {
    for (word, constant) in state.iter_mut().zip(rc.iter()) {
        *word = S::sbox(*word + constant);
    }
    poseidon_apply_mds::<F, T>(state, mds);
}

fn poseidon_partial_round<F: Field, S: Spec<F, T, RATE>, const T: usize, const RATE: usize>(
    state: &mut State<F, T>,
    mds: &Mds<F, T>,
    rc: &[F; T],
) {
    for (word, constant) in state.iter_mut().zip(rc.iter()) {
        *word += constant;
    }
    state[0] = S::sbox(state[0]);
    poseidon_apply_mds::<F, T>(state, mds);
}

fn poseidon_apply_mds<F: Field, const T: usize>(state: &mut State<F, T>, mds: &Mds<F, T>) {
    let mut next = [F::ZERO; T];
    for row in 0..T {
        for col in 0..T {
            next[row] += mds[row][col] * state[col];
        }
    }
    *state = next;
}

fn hash_to_field<F: PrimeField<Repr = [u8; 32]>>(
    domain: &[u8],
    dst: &[u8],
    msg: &[u8],
) -> Result<F, AssetOrchardError> {
    let mut counter = 0u32;
    loop {
        let digest = hash_to_field_digest(domain, dst, msg, counter)?;
        let bytes: [u8; 32] = digest[0..32]
            .try_into()
            .map_err(|_| AssetOrchardError::new("digest_slice", "invalid field digest slice"))?;
        if let Some(field) = Option::<F>::from(F::from_repr(bytes)) {
            return Ok(field);
        }
        counter = counter.checked_add(1).ok_or_else(|| {
            AssetOrchardError::new(
                "hash_to_field_exhausted",
                "hash-to-field counter overflowed",
            )
        })?;
    }
}

fn loop_hash_to_scalar(dst: &[u8], msg: &[u8]) -> Result<pallas::Scalar, AssetOrchardError> {
    let mut counter = 0u32;
    loop {
        let digest = hash_to_field_digest(HASH_TO_PALLAS_SCALAR_DOMAIN, dst, msg, counter)?;
        let bytes: [u8; 32] = digest[0..32]
            .try_into()
            .map_err(|_| AssetOrchardError::new("digest_slice", "invalid scalar digest slice"))?;
        if let Some(scalar) = Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(bytes)) {
            if !bool::from(scalar.is_zero()) {
                return Ok(scalar);
            }
        }
        counter = counter.checked_add(1).ok_or_else(|| {
            AssetOrchardError::new(
                "hash_to_scalar_exhausted",
                "hash-to-scalar counter overflowed",
            )
        })?;
    }
}

fn hash_to_field_digest(
    domain: &[u8],
    dst: &[u8],
    msg: &[u8],
    counter: u32,
) -> Result<[u8; 64], AssetOrchardError> {
    let mut hasher = Sha3_512::new();
    Digest::update(&mut hasher, domain);
    append_len_bytes(&mut hasher, dst)?;
    append_len_bytes(&mut hasher, msg)?;
    Digest::update(&mut hasher, counter.to_le_bytes());
    let digest = hasher.finalize();
    digest
        .as_slice()
        .try_into()
        .map_err(|_| AssetOrchardError::new("digest_slice", "invalid SHA3-512 digest length"))
}

fn extract_p(point: pallas::Point) -> Result<pallas::Base, AssetOrchardError> {
    let affine = point.to_affine();
    let coordinates: Option<Coordinates<pallas::Affine>> = Option::from(affine.coordinates());
    coordinates
        .map(|coordinates| *coordinates.x())
        .ok_or_else(|| {
            AssetOrchardError::new(
                "invalid_commitment_point",
                "asset note commitment point must not be identity",
            )
        })
}

fn point_bits(point: pallas::Affine) -> Result<Vec<bool>, AssetOrchardError> {
    let bytes = point_enc(point)?;
    Ok(bytes_to_le_bits(&bytes, ASSET_ORCHARD_POINT_BYTES * 8))
}

fn i2lebsp_from_field(field: pallas::Base, bit_len: usize) -> Vec<bool> {
    bytes_to_le_bits(&field_enc(field), bit_len)
}

fn i2lebsp_from_u128(value: u128, bit_len: usize) -> Vec<bool> {
    bytes_to_le_bits(&value.to_le_bytes(), bit_len)
}

fn i2lebsp_from_u64(value: u64, bit_len: usize) -> Vec<bool> {
    bytes_to_le_bits(&value.to_le_bytes(), bit_len)
}

fn bytes_to_le_bits(bytes: &[u8], bit_len: usize) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bit_len);
    for bit_index in 0..bit_len {
        let byte = bytes[bit_index / 8];
        let bit = (byte >> (bit_index % 8)) & 1;
        bits.push(bit == 1);
    }
    bits
}

fn reject_identity_point(
    label: &'static str,
    point: pallas::Affine,
) -> Result<(), AssetOrchardError> {
    if bool::from(point.is_identity()) {
        return Err(AssetOrchardError::new(
            "identity_point",
            format!("{label} must not be the identity point"),
        ));
    }
    Ok(())
}

fn validate_canonical_text(
    label: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), AssetOrchardError> {
    validate_canonical_bytes(label, value.as_bytes(), max_bytes)?;
    if value.trim() != value {
        return Err(AssetOrchardError::new(
            "noncanonical_text",
            format!("{label} has leading or trailing whitespace"),
        ));
    }
    Ok(())
}

fn validate_private_egress_exit_fields(
    chain_id: &str,
    pool_id: &str,
    protocol_version: u32,
    to: &str,
    asset_id: &str,
    amount: u64,
    fee: u64,
    policy_id: &str,
    disclosure_hash: &str,
) -> Result<(), AssetOrchardError> {
    validate_canonical_text("chain_id", chain_id, 256)?;
    validate_canonical_text("pool_id", pool_id, ASSET_ORCHARD_MAX_POOL_ID_BYTES)?;
    validate_canonical_text("asset_orchard_private_egress_to", to, 256)?;
    validate_canonical_text(
        "asset_orchard_private_egress_asset_id",
        asset_id,
        ASSET_ORCHARD_MAX_ASSET_ID_BYTES,
    )?;
    validate_canonical_text("asset_orchard_private_egress_policy_id", policy_id, 256)?;
    validate_canonical_text(
        "asset_orchard_private_egress_disclosure_hash",
        disclosure_hash,
        256,
    )?;
    if pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_pool",
            format!("unsupported asset-orchard pool `{pool_id}`"),
        ));
    }
    if protocol_version == 0 {
        return Err(AssetOrchardError::new(
            "invalid_protocol_version",
            "protocol_version must be nonzero",
        ));
    }
    if amount == 0 {
        return Err(AssetOrchardError::new(
            "zero_private_egress_amount",
            "asset-orchard private egress amount must be nonzero",
        ));
    }
    if fee != 0 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_private_egress_fee",
            "asset-orchard private egress v1 requires fee 0",
        ));
    }
    Ok(())
}

fn text_to_field(label: &str, text: &str) -> Result<pallas::Base, AssetOrchardError> {
    validate_canonical_text("private_egress_hash_label", label, 256)?;
    hash_to_pallas_base(label, text.as_bytes())
}

fn bytes_to_field(label: &str, bytes: &[u8]) -> Result<pallas::Base, AssetOrchardError> {
    validate_canonical_text("private_egress_hash_label", label, 256)?;
    hash_to_pallas_base(label, bytes)
}

fn validate_canonical_bytes(
    label: &'static str,
    value: &[u8],
    max_bytes: usize,
) -> Result<(), AssetOrchardError> {
    if value.is_empty() {
        return Err(AssetOrchardError::new(
            "empty_canonical_bytes",
            format!("{label} must not be empty"),
        ));
    }
    if value.len() > max_bytes {
        return Err(AssetOrchardError::new(
            "oversized_canonical_bytes",
            format!("{label} has {} bytes, max {max_bytes}", value.len()),
        ));
    }
    if value.iter().any(|byte| byte.is_ascii_control()) {
        return Err(AssetOrchardError::new(
            "invalid_canonical_bytes",
            format!("{label} contains control bytes"),
        ));
    }
    Ok(())
}

fn append_len_bytes<H: Update>(hasher: &mut H, bytes: &[u8]) -> Result<(), AssetOrchardError> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| AssetOrchardError::new("len_bytes_overflow", "length does not fit u32le"))?;
    hasher.update(&len.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

fn append_len_bytes_vec(payload: &mut Vec<u8>, bytes: &[u8]) -> Result<(), AssetOrchardError> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| AssetOrchardError::new("len_bytes_overflow", "length does not fit u32le"))?;
    payload.extend_from_slice(&len.to_le_bytes());
    payload.extend_from_slice(bytes);
    Ok(())
}

fn parse_pallas_base(value: &str) -> Result<pallas::Base, AssetOrchardError> {
    let bytes = fixed_lower_hex_array::<ASSET_ORCHARD_FIELD_BYTES>("asset_orchard_field", value)?;
    Option::<pallas::Base>::from(pallas::Base::from_repr(bytes)).ok_or_else(|| {
        AssetOrchardError::new(
            "noncanonical_pallas_field",
            "field element is not canonical Pallas-base encoding",
        )
    })
}

fn parse_pallas_scalar(value: &str) -> Result<pallas::Scalar, AssetOrchardError> {
    let bytes = fixed_lower_hex_array::<ASSET_ORCHARD_FIELD_BYTES>("asset_orchard_scalar", value)?;
    let scalar =
        Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(bytes)).ok_or_else(|| {
            AssetOrchardError::new(
                "noncanonical_pallas_scalar",
                "scalar element is not canonical Pallas-scalar encoding",
            )
        })?;
    if bool::from(scalar.is_zero()) {
        return Err(AssetOrchardError::new(
            "zero_pallas_scalar",
            "asset-orchard scalar must be nonzero",
        ));
    }
    Ok(scalar)
}

fn parse_pallas_point(value: &str) -> Result<pallas::Affine, AssetOrchardError> {
    let bytes = fixed_lower_hex_array::<ASSET_ORCHARD_POINT_BYTES>("asset_orchard_point", value)?;
    let point =
        Option::<pallas::Affine>::from(pallas::Affine::from_bytes(&bytes)).ok_or_else(|| {
            AssetOrchardError::new(
                "invalid_pallas_point",
                "point is not a canonical Pallas point encoding",
            )
        })?;
    reject_identity_point("asset_orchard_point", point)?;
    Ok(point)
}

fn parse_fixed_lower_hex(
    label: &'static str,
    value: String,
    expected_bytes: usize,
) -> Result<String, AssetOrchardError> {
    parse_lower_hex(label, &value, expected_bytes, expected_bytes)?;
    Ok(value)
}

fn parse_lower_hex(
    label: &'static str,
    value: &str,
    min_bytes: usize,
    max_bytes: usize,
) -> Result<(), AssetOrchardError> {
    if value.is_empty() {
        return Err(AssetOrchardError::new(
            "empty_hex",
            format!("{label} hex must not be empty"),
        ));
    }
    if value.trim() != value {
        return Err(AssetOrchardError::new(
            "noncanonical_hex",
            format!("{label} hex has leading or trailing whitespace"),
        ));
    }
    if !value.len().is_multiple_of(2) {
        return Err(AssetOrchardError::new(
            "noncanonical_hex",
            format!("{label} hex has odd length"),
        ));
    }
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(AssetOrchardError::new(
            "noncanonical_hex",
            format!("{label} hex must be lowercase"),
        ));
    }
    let byte_len = value.len() / 2;
    if byte_len < min_bytes {
        return Err(AssetOrchardError::new(
            "undersized_hex",
            format!("{label} has {byte_len} bytes, min {min_bytes}"),
        ));
    }
    if byte_len > max_bytes {
        return Err(AssetOrchardError::new(
            "oversized_hex",
            format!("{label} has {byte_len} bytes, max {max_bytes}"),
        ));
    }
    hex_to_bytes(value).map_err(|error| {
        AssetOrchardError::new("invalid_hex", format!("{label} has invalid hex: {error}"))
    })?;
    Ok(())
}

fn fixed_lower_hex_array<const N: usize>(
    label: &'static str,
    value: &str,
) -> Result<[u8; N], AssetOrchardError> {
    parse_lower_hex(label, value, N, N)?;
    let bytes = hex_to_bytes(value).map_err(|error| {
        AssetOrchardError::new("invalid_hex", format!("{label} has invalid hex: {error}"))
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        AssetOrchardError::new(
            "invalid_hex_length",
            format!("{label} decoded to {} bytes, expected {N}", bytes.len()),
        )
    })
}

fn validate_count(
    label: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), AssetOrchardError> {
    if actual != expected {
        return Err(AssetOrchardError::new(
            "action_count_mismatch",
            format!("{label} count {actual} does not match expected count {expected}"),
        ));
    }
    Ok(())
}

fn validate_accounting_record_set(
    label: &'static str,
    records: &[AssetOrchardSwapAccountingRecord],
) -> Result<(), AssetOrchardError> {
    if has_duplicate_strings(
        records
            .iter()
            .map(|record| record.output_commitment.as_str()),
    ) {
        return Err(AssetOrchardError::new(
            "duplicate_asset_orchard_accounting_commitment",
            format!("{label} accounting records contain duplicate output commitments"),
        ));
    }
    for record in records {
        validate_accounting_record(label, record)?;
    }
    Ok(())
}

fn validate_accounting_record(
    label: &'static str,
    record: &AssetOrchardSwapAccountingRecord,
) -> Result<(), AssetOrchardError> {
    fixed_lower_hex_array::<ASSET_ORCHARD_FIELD_BYTES>(
        "asset_orchard_accounting_output_commitment",
        &record.output_commitment,
    )?;
    record.value_commitment.to_affine().map_err(|error| {
        AssetOrchardError::new(
            "invalid_asset_orchard_accounting_value_commitment",
            format!("{label} accounting value commitment is invalid: {error}"),
        )
    })?;
    Ok(())
}

fn accounting_sum(
    records: &[AssetOrchardSwapAccountingRecord],
) -> Result<[u8; ASSET_ORCHARD_POINT_BYTES], AssetOrchardError> {
    let mut total = pallas::Point::identity();
    for record in records {
        total += pallas::Point::from(record.value_commitment.to_affine()?);
    }
    Ok(total.to_affine().to_bytes())
}

fn has_duplicate_hex<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = Vec::<&'a str>::new();
    for value in values {
        if seen.iter().any(|existing| *existing == value) {
            return true;
        }
        seen.push(value);
    }
    false
}

fn has_duplicate_strings<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = Vec::<&'a str>::new();
    for value in values {
        if seen.iter().any(|existing| *existing == value) {
            return true;
        }
        seen.push(value);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::PrimeField;
    use orchard::primitives::redpallas::{SigningKey, SpendAuth, VerificationKey};
    use pasta_curves::arithmetic::CurveExt;
    use pasta_curves::group::ff::Field;
    use rand::rngs::OsRng;

    fn sample_point(seed: &[u8]) -> pallas::Affine {
        pallas::Point::hash_to_curve("postfiat.asset_orchard.test_point")(seed).to_affine()
    }

    fn sample_note(asset_id: &str, value: u64, rho: pallas::Base) -> AssetNoteOpening {
        let rseed = [9u8; ASSET_ORCHARD_RSEED_BYTES];
        AssetNoteOpening {
            diversifier: [7u8; ASSET_ORCHARD_DIVERSIFIER_BYTES],
            g_d: sample_point(b"g_d"),
            pk_d: sample_point(b"pk_d"),
            asset_tag: AssetTag::derive(asset_id).expect("asset tag"),
            value,
            rho,
            psi: orchard_psi(&rseed, rho).expect("psi"),
            rcm: orchard_rcm(&rseed, rho).expect("rcm"),
        }
    }

    fn asset_note_source_bits(
        source: AssetNoteMessageSource,
        pool_domain: pallas::Base,
        note: &AssetNoteOpening,
    ) -> Vec<bool> {
        match source {
            AssetNoteMessageSource::PoolDomain => i2lebsp_from_field(pool_domain, 255),
            AssetNoteMessageSource::AssetTagLo => i2lebsp_from_u128(note.asset_tag.lo, 128),
            AssetNoteMessageSource::AssetTagHi => i2lebsp_from_u128(note.asset_tag.hi, 128),
            AssetNoteMessageSource::GdX => point_bits(note.g_d)
                .expect("g_d bits")
                .into_iter()
                .take(255)
                .collect(),
            AssetNoteMessageSource::GdYSign => point_bits(note.g_d)
                .expect("g_d bits")
                .into_iter()
                .skip(255)
                .take(1)
                .collect(),
            AssetNoteMessageSource::PkdX => point_bits(note.pk_d)
                .expect("pk_d bits")
                .into_iter()
                .take(255)
                .collect(),
            AssetNoteMessageSource::PkdYSign => point_bits(note.pk_d)
                .expect("pk_d bits")
                .into_iter()
                .skip(255)
                .take(1)
                .collect(),
            AssetNoteMessageSource::Value => i2lebsp_from_u64(note.value, 64),
            AssetNoteMessageSource::Rho => i2lebsp_from_field(note.rho, 255),
            AssetNoteMessageSource::Psi => i2lebsp_from_field(note.psi, 255),
            AssetNoteMessageSource::Padding => vec![false; 3],
        }
    }

    fn sample_action() -> AssetOrchardSwapAction {
        let chain_id = "postfiat-wan-devnet";
        let genesis_hash = [1u8; 32];
        let protocol_version = 2;
        let pool =
            AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)
                .expect("pool domain");
        let anchor = hash_to_pallas_base("test", b"anchor").expect("anchor");
        let nullifiers = [
            hash_to_pallas_base("test", b"nf0").expect("nf0"),
            hash_to_pallas_base("test", b"nf1").expect("nf1"),
        ];
        let rk_points = [sample_point(b"rk0"), sample_point(b"rk1")];
        let output_commitments = [
            hash_to_pallas_base("test", b"cmx0").expect("cmx0"),
            hash_to_pallas_base("test", b"cmx1").expect("cmx1"),
        ];
        let encrypted_outputs = [
            AssetOrchardBoundedBytes::from_bytes(
                b"encrypted-output-0",
                ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
            )
            .expect("eo0"),
            AssetOrchardBoundedBytes::from_bytes(
                b"encrypted-output-1",
                ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
            )
            .expect("eo1"),
        ];
        let pricing_claim = sample_pricing_claim();
        let fields = AssetOrchardActionPublicFields {
            pool_domain: pool,
            anchor,
            nullifiers,
            randomized_verification_keys: [
                RandomizedVerificationKeyFields::from_affine(rk_points[0]).expect("rk0"),
                RandomizedVerificationKeyFields::from_affine(rk_points[1]).expect("rk1"),
            ],
            output_commitments,
            encrypted_output_hashes: [
                encrypted_output_hash(0, &encrypted_outputs[0].to_bytes().expect("eo0 bytes"))
                    .expect("eo0 hash"),
                encrypted_output_hash(1, &encrypted_outputs[1].to_bytes().expect("eo1 bytes"))
                    .expect("eo1 hash"),
            ],
            pricing: sample_pricing_public_fields(),
            fee: 0,
        };
        AssetOrchardSwapAction {
            version: ASSET_ORCHARD_ACTION_VERSION_V1,
            schema: ASSET_ORCHARD_ACTION_SCHEMA_V1.to_string(),
            pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
            proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
            circuit_id: ASSET_ORCHARD_CIRCUIT_ID_V1.to_string(),
            pool_domain: AssetOrchardFieldElement::from_field(pool),
            anchor: AssetOrchardFieldElement::from_field(anchor),
            nullifiers: nullifiers
                .into_iter()
                .map(AssetOrchardFieldElement::from_field)
                .collect(),
            randomized_verification_keys: rk_points
                .into_iter()
                .map(|point| AssetOrchardPoint::from_affine(point).expect("rk point"))
                .collect(),
            output_commitments: output_commitments
                .into_iter()
                .map(AssetOrchardFieldElement::from_field)
                .collect(),
            encrypted_outputs: encrypted_outputs.into_iter().collect(),
            accounting_inputs: sample_accounting_input_records().into_iter().collect(),
            accounting_outputs: sample_accounting_output_records(output_commitments)
                .into_iter()
                .collect(),
            pricing_claim,
            swap_binding_hash: AssetOrchardSwapBindingHash::from_bytes(
                &swap_binding_hash(&fields).expect("binding"),
            ),
            fee: 0,
            proof: AssetOrchardProofBytes::from_bytes(b"placeholder-proof").expect("proof"),
            spend_authorization_signatures: vec![
                AssetOrchardSpendAuthSignature::parse_hex(
                    "11".repeat(ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES),
                )
                .expect("sig0"),
                AssetOrchardSpendAuthSignature::parse_hex(
                    "22".repeat(ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES),
                )
                .expect("sig1"),
            ],
        }
    }

    fn sample_pricing_claim() -> AssetOrchardPricingClaim {
        let base = AssetTag::derive("a651").expect("base tag");
        let quote = AssetTag::derive("pfUSDC").expect("quote tag");
        AssetOrchardPricingClaim {
            nav_epoch: 59,
            reserve_packet_hash: "ab".repeat(48),
            ratio_numerator: 9,
            ratio_denominator: 5,
            mode: "at_nav_with_band".to_string(),
            band_bps: 0,
            base_asset_tag_lo: base.lo,
            base_asset_tag_hi: base.hi,
            quote_asset_tag_lo: quote.lo,
            quote_asset_tag_hi: quote.hi,
        }
    }

    fn sample_pricing_public_fields() -> AssetOrchardPricingPublicFields {
        let claim = sample_pricing_claim();
        AssetOrchardPricingPublicFields {
            base_asset_tag: AssetTag {
                lo: claim.base_asset_tag_lo,
                hi: claim.base_asset_tag_hi,
            },
            quote_asset_tag: AssetTag {
                lo: claim.quote_asset_tag_lo,
                hi: claim.quote_asset_tag_hi,
            },
            ratio_numerator: claim.ratio_numerator,
            ratio_denominator: claim.ratio_denominator,
            commitment: claim.commitment_fields().expect("pricing commitment"),
        }
    }

    fn sample_accounting_record(
        output_commitment: pallas::Base,
        asset_id: &str,
        amount: u64,
        blinding_seed: &[u8],
    ) -> AssetOrchardSwapAccountingRecord {
        let tag = AssetTag::derive(asset_id).expect("asset tag");
        let output_commitment = AssetOrchardFieldElement::from_field(output_commitment);
        let blinding = hash_to_pallas_scalar_nonzero(
            "postfiat.asset_orchard.test.accounting_blinding",
            blinding_seed,
        )
        .expect("accounting blinding");
        asset_orchard_accounting_record(&output_commitment, tag, amount, blinding)
            .expect("accounting record")
    }

    fn sample_accounting_input_records() -> [AssetOrchardSwapAccountingRecord; 2] {
        [
            sample_accounting_record(
                hash_to_pallas_base("test", b"input-cmx0").expect("input cmx0"),
                "a651",
                5,
                b"a651",
            ),
            sample_accounting_record(
                hash_to_pallas_base("test", b"input-cmx1").expect("input cmx1"),
                "pfUSDC",
                9,
                b"pfUSDC",
            ),
        ]
    }

    fn sample_accounting_output_records(
        output_commitments: [pallas::Base; 2],
    ) -> [AssetOrchardSwapAccountingRecord; 2] {
        [
            sample_accounting_record(output_commitments[0], "pfUSDC", 9, b"pfUSDC"),
            sample_accounting_record(output_commitments[1], "a651", 5, b"a651"),
        ]
    }

    #[test]
    fn swap_accounting_serialization_hides_asset_identity() {
        let tag = AssetTag::derive("a651").expect("asset tag");
        let deterministic_asset_generator = AssetOrchardPoint::from_affine(
            accounting_asset_generator(tag)
                .expect("generator")
                .to_affine(),
        )
        .expect("generator point")
        .as_hex()
        .to_string();
        let output_commitment =
            hash_to_pallas_base("test", b"same-asset-output-cmx").expect("output cmx");
        let first = sample_accounting_record(output_commitment, "a651", 5, b"first-blinding");
        let second = sample_accounting_record(output_commitment, "a651", 5, b"second-blinding");

        assert_ne!(
            first.value_commitment, second.value_commitment,
            "same-asset same-amount accounting must not serialize a stable value commitment"
        );
        let json = serde_json::to_string(&[first, second]).expect("accounting json");
        for forbidden in [
            "asset_commitment",
            "asset_tag_lo",
            "asset_tag_hi",
            "amount",
            deterministic_asset_generator.as_str(),
        ] {
            assert!(
                !json.contains(forbidden),
                "swap accounting leaked linkable asset data `{forbidden}`: {json}"
            );
        }
    }

    fn spend_signing_key(seed: u64) -> SigningKey<SpendAuth> {
        SigningKey::try_from(pallas::Scalar::from(seed).to_repr()).expect("nonzero spend key")
    }

    fn signed_sample_action() -> AssetOrchardSwapAction {
        let chain_id = "postfiat-wan-devnet";
        let genesis_hash = [1u8; 32];
        let protocol_version = 2;
        let mut action = sample_action();
        let signing_keys = [spend_signing_key(7), spend_signing_key(11)];
        let verification_keys = [
            VerificationKey::from(&signing_keys[0]),
            VerificationKey::from(&signing_keys[1]),
        ];
        action.randomized_verification_keys = verification_keys
            .iter()
            .map(|key| {
                let bytes: [u8; ASSET_ORCHARD_POINT_BYTES] = key.into();
                AssetOrchardPoint::parse_hex(bytes_to_hex(&bytes)).expect("rk")
            })
            .collect();
        let fields = action
            .public_fields_without_binding_check()
            .expect("public fields");
        action.swap_binding_hash =
            AssetOrchardSwapBindingHash::from_bytes(&swap_binding_hash(&fields).expect("binding"));
        let sighash = action
            .sighash(chain_id, genesis_hash, protocol_version)
            .expect("sighash");
        action.spend_authorization_signatures = signing_keys
            .iter()
            .map(|key| AssetOrchardSpendAuthSignature::from_orchard(&key.sign(OsRng, &sighash)))
            .collect();
        action
    }

    #[test]
    fn private_egress_action_serialization_does_not_disclose_note_opening() {
        let chain_id = "postfiat-wan-devnet";
        let genesis_hash = [2u8; 32];
        let protocol_version = 3;
        let pool_domain =
            AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)
                .expect("pool domain");
        let anchor = hash_to_pallas_base("test", b"private-egress-anchor").expect("anchor");
        let nullifier = hash_to_pallas_base("test", b"private-egress-nullifier").expect("nf");
        let rk_point = sample_point(b"private-egress-rk");
        let tag = AssetTag::derive("a651").expect("asset tag");
        let exit_binding_hash = asset_orchard_private_egress_exit_binding_hash(
            &AssetOrchardPrivateEgressExitBindingPreimage {
                chain_id,
                genesis_hash,
                protocol_version,
                pool_id: ASSET_ORCHARD_POOL_ID_V1,
                circuit_id: ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
                pool_domain,
                to: "alice",
                asset_id: "a651",
                amount: 5,
                fee: 0,
                policy_id: "postfiat.asset_orchard.private_egress.test",
                disclosure_hash: "test-disclosure",
            },
        )
        .expect("exit binding");
        let action = AssetOrchardPrivateEgressAction {
            version: ASSET_ORCHARD_ACTION_VERSION_V1,
            schema: ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1.to_string(),
            pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
            proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
            circuit_id: ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1.to_string(),
            pool_domain: AssetOrchardFieldElement::from_field(pool_domain),
            anchor: AssetOrchardFieldElement::from_field(anchor),
            nullifier: AssetOrchardFieldElement::from_field(nullifier),
            randomized_verification_key: AssetOrchardPoint::from_affine(rk_point).expect("rk"),
            asset_tag_lo: tag.lo,
            asset_tag_hi: tag.hi,
            amount: 5,
            fee: 0,
            exit_binding_hash: AssetOrchardSwapBindingHash::from_bytes(&exit_binding_hash),
            proof: AssetOrchardProofBytes::from_bytes(b"placeholder-proof").expect("proof"),
            spend_authorization_signature: AssetOrchardSpendAuthSignature::parse_hex(
                "33".repeat(ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES),
            )
            .expect("signature"),
        };
        action.validate().expect("private egress action validates");
        let public_instance = action.public_instance().expect("public instance");
        assert_eq!(
            public_instance.len(),
            ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN
        );

        let json = serde_json::to_string(&action).expect("json");
        for forbidden in [
            "\"note\"",
            "\"nk\"",
            "\"rivk\"",
            "\"output_commitment\"",
            "\"spend_auth_randomizer\"",
            "\"diversifier\"",
            "\"rho\"",
            "\"psi\"",
            "\"rcm\"",
        ] {
            assert!(
                !json.contains(forbidden),
                "private egress action leaked forbidden field {forbidden}: {json}"
            );
        }
    }

    #[test]
    fn hash_to_pallas_base_is_deterministic_and_domain_separated() {
        let a = hash_to_pallas_base("domain-a", b"message").expect("field a");
        let a_again = hash_to_pallas_base("domain-a", b"message").expect("field a again");
        let b = hash_to_pallas_base("domain-b", b"message").expect("field b");

        assert_eq!(a, a_again);
        assert_ne!(a, b);
        assert_eq!(field_enc(a).len(), ASSET_ORCHARD_FIELD_BYTES);
    }

    #[test]
    fn hash_to_pallas_scalar_rejects_zero_by_construction() {
        let scalar = hash_to_pallas_scalar_nonzero("domain-a", b"message").expect("nonzero scalar");
        assert!(!bool::from(scalar.is_zero()));
        assert_eq!(scalar_enc(scalar).len(), ASSET_ORCHARD_FIELD_BYTES);
    }

    #[test]
    fn asset_tag_uses_first_256_bits_and_rejects_noncanonical_text() {
        let tag = AssetTag::derive("a651").expect("asset tag");
        let tag_again = AssetTag::derive_from_canonical_bytes(b"a651").expect("asset tag bytes");
        let other = AssetTag::derive("a652").expect("other tag");

        assert_eq!(tag, tag_again);
        assert_ne!(tag, other);
        assert!(tag.lo > 0 || tag.hi > 0);
        assert_eq!(
            AssetTag::derive(" a651")
                .expect_err("space rejected")
                .code(),
            "noncanonical_text"
        );
    }

    #[test]
    fn pool_domain_binds_chain_genesis_protocol_pool_and_note_version() {
        let input = PoolDomainInput {
            chain_id: "postfiat-wan-devnet",
            genesis_hash: [1u8; 32],
            protocol_version: 2,
            pool_id: ASSET_ORCHARD_POOL_ID_V1,
            note_version: ASSET_ORCHARD_NOTE_VERSION_V1,
        };
        let domain = pool_domain(&input).expect("pool domain");
        let mut changed = input.clone();
        changed.protocol_version = 3;
        let other = pool_domain(&changed).expect("changed pool domain");

        assert_ne!(domain, other);
    }

    #[test]
    fn orchard_psi_and_rcm_bind_rseed_and_rho() {
        let rseed = [3u8; ASSET_ORCHARD_RSEED_BYTES];
        let rho = pallas::Base::from(42);
        let psi = orchard_psi(&rseed, rho).expect("psi");
        let rcm = orchard_rcm(&rseed, rho).expect("rcm");
        let psi_changed = orchard_psi(&rseed, pallas::Base::from(43)).expect("psi changed");

        assert_ne!(psi, psi_changed);
        assert!(!bool::from(rcm.is_zero()));
    }

    #[test]
    fn asset_note_message_segments_reconstruct_host_message_bits() {
        let pool = hash_to_pallas_base("test", b"pool").expect("pool");
        let rho = hash_to_pallas_base("test", b"rho").expect("rho");
        let note = sample_note("a651", 123, rho);
        let host_bits = asset_note_message_bits(pool, &note).expect("host bits");
        let segments = asset_note_message_segments();
        let mut pieces = vec![Vec::<bool>::new(); ASSET_ORCHARD_NOTE_MESSAGE_PIECE_COUNT];

        for segment in segments {
            assert_eq!(pieces[segment.piece_index].len(), segment.piece_bit_offset);
            let source_bits = asset_note_source_bits(segment.source, pool, &note);
            pieces[segment.piece_index].extend_from_slice(
                &source_bits
                    [segment.source_bit_offset..segment.source_bit_offset + segment.bit_len],
            );
        }

        for (index, piece) in pieces.iter().enumerate() {
            let expected_len = if index + 1 == ASSET_ORCHARD_NOTE_MESSAGE_PIECE_COUNT {
                ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS
                    - (ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS * index)
            } else {
                ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS
            };
            assert_eq!(piece.len(), expected_len);
        }
        let rebuilt = pieces.into_iter().flatten().collect::<Vec<_>>();
        assert_eq!(rebuilt.len(), ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS);
        assert_eq!(&rebuilt[..host_bits.len()], host_bits.as_slice());
        assert!(rebuilt[host_bits.len()..].iter().all(|bit| !*bit));
        assert_eq!(host_bits.len(), ASSET_ORCHARD_NOTE_MESSAGE_BITS);
    }

    #[test]
    fn asset_nullifier_and_output_rho_are_domain_separated() {
        let pool = hash_to_pallas_base("test", b"pool").expect("pool");
        let anchor = hash_to_pallas_base("test", b"anchor").expect("anchor");
        let nf0 = hash_to_pallas_base("test", b"nf0").expect("nf0");
        let nf1 = hash_to_pallas_base("test", b"nf1").expect("nf1");
        let rk0 = RandomizedVerificationKeyFields::from_affine(sample_point(b"rk0")).expect("rk0");
        let rk1 = RandomizedVerificationKeyFields::from_affine(sample_point(b"rk1")).expect("rk1");
        let cmx = hash_to_pallas_base("test", b"cmx").expect("cmx");

        let nf = asset_derive_nullifier(
            pool,
            hash_to_pallas_base("test", b"nk").expect("nk"),
            hash_to_pallas_base("test", b"rho").expect("rho"),
            hash_to_pallas_base("test", b"psi").expect("psi"),
            cmx,
        )
        .expect("asset nullifier");
        let rho0 = asset_output_rho(pool, anchor, [nf0, nf1], [rk0, rk1], 0).expect("rho 0");
        let rho1 = asset_output_rho(pool, anchor, [nf0, nf1], [rk0, rk1], 1).expect("rho 1");

        assert_ne!(nf, rho0);
        assert_ne!(rho0, rho1);
    }

    #[test]
    fn encrypted_output_hash_returns_three_128_bit_limbs() {
        let hash = encrypted_output_hash(0, b"ciphertext").expect("encrypted output hash");
        let fields = hash.as_fields();

        assert_ne!(fields[0], fields[1]);
        assert_eq!(
            encrypted_output_hash(2, b"ciphertext")
                .expect_err("bad index")
                .code(),
            "invalid_output_index"
        );
    }

    #[test]
    fn asset_note_commitment_binds_pool_asset_value_and_points() {
        let pool = hash_to_pallas_base("test", b"pool").expect("pool");
        let other_pool = hash_to_pallas_base("test", b"pool-2").expect("pool 2");
        let rho = pallas::Base::from(77);
        let note = sample_note("a651", 100, rho);
        let mut other_asset = note.clone();
        other_asset.asset_tag = AssetTag::derive("a652").expect("other tag");
        let mut other_value = note.clone();
        other_value.value = 101;

        let cmx = note.cmx(pool).expect("cmx");
        assert_ne!(cmx, note.cmx(other_pool).expect("other pool cmx"));
        assert_ne!(cmx, other_asset.cmx(pool).expect("other asset cmx"));
        assert_ne!(cmx, other_value.cmx(pool).expect("other value cmx"));

        let bits = asset_note_message_bits(pool, &note).expect("message bits");
        assert_eq!(bits.len(), 1597);
    }

    #[test]
    fn h_action_and_swap_binding_hash_are_consistent() {
        let pool = hash_to_pallas_base("test", b"pool").expect("pool");
        let anchor = hash_to_pallas_base("test", b"anchor").expect("anchor");
        let nf0 = hash_to_pallas_base("test", b"nf0").expect("nf0");
        let nf1 = hash_to_pallas_base("test", b"nf1").expect("nf1");
        let rk0 = RandomizedVerificationKeyFields::from_affine(sample_point(b"rk0")).expect("rk0");
        let rk1 = RandomizedVerificationKeyFields::from_affine(sample_point(b"rk1")).expect("rk1");
        let fields = AssetOrchardActionPublicFields {
            pool_domain: pool,
            anchor,
            nullifiers: [nf0, nf1],
            randomized_verification_keys: [rk0, rk1],
            output_commitments: [
                hash_to_pallas_base("test", b"cmx0").expect("cmx0"),
                hash_to_pallas_base("test", b"cmx1").expect("cmx1"),
            ],
            encrypted_output_hashes: [
                encrypted_output_hash(0, b"eo0").expect("eo0"),
                encrypted_output_hash(1, b"eo1").expect("eo1"),
            ],
            pricing: sample_pricing_public_fields(),
            fee: 0,
        };

        let action = h_action(&fields).expect("h action");
        let binding = swap_binding_hash(&fields).expect("binding hash");
        assert_eq!(&binding[0..32], &field_enc(action[0]));
        assert_eq!(&binding[32..64], &field_enc(action[1]));
        assert_eq!(
            fields.public_instance().expect("instance").len(),
            ASSET_ORCHARD_PUBLIC_INSTANCE_LEN
        );
    }

    #[test]
    fn h_sig_binds_raw_ciphertext_and_binding_hash() {
        let pool = hash_to_pallas_base("test", b"pool").expect("pool");
        let anchor = hash_to_pallas_base("test", b"anchor").expect("anchor");
        let nf0 = hash_to_pallas_base("test", b"nf0").expect("nf0");
        let nf1 = hash_to_pallas_base("test", b"nf1").expect("nf1");
        let rk0 = sample_point(b"rk0");
        let rk1 = sample_point(b"rk1");
        let fields = AssetOrchardActionPublicFields {
            pool_domain: pool,
            anchor,
            nullifiers: [nf0, nf1],
            randomized_verification_keys: [
                RandomizedVerificationKeyFields::from_affine(rk0).expect("rk0"),
                RandomizedVerificationKeyFields::from_affine(rk1).expect("rk1"),
            ],
            output_commitments: [
                hash_to_pallas_base("test", b"cmx0").expect("cmx0"),
                hash_to_pallas_base("test", b"cmx1").expect("cmx1"),
            ],
            encrypted_output_hashes: [
                encrypted_output_hash(0, b"eo0").expect("eo0"),
                encrypted_output_hash(1, b"eo1").expect("eo1"),
            ],
            pricing: sample_pricing_public_fields(),
            fee: 0,
        };
        let binding = swap_binding_hash(&fields).expect("binding");
        let accounting_inputs = sample_accounting_input_records();
        let accounting_outputs = sample_accounting_output_records(fields.output_commitments);
        let preimage = AssetOrchardSigPreimage {
            chain_id: "postfiat-wan-devnet",
            genesis_hash: [1u8; 32],
            protocol_version: 2,
            pool_id: ASSET_ORCHARD_POOL_ID_V1,
            circuit_id: ASSET_ORCHARD_CIRCUIT_ID_V1,
            pool_domain: pool,
            anchor,
            nullifiers: [nf0, nf1],
            randomized_verification_keys: [rk0, rk1],
            output_commitments: fields.output_commitments,
            encrypted_outputs: [b"eo0".as_slice(), b"eo1".as_slice()],
            accounting_inputs: [&accounting_inputs[0], &accounting_inputs[1]],
            accounting_outputs: [&accounting_outputs[0], &accounting_outputs[1]],
            swap_binding_hash: binding,
            fee: 0,
        };
        let sig = h_sig(&preimage).expect("sig hash");
        let mut changed = preimage.clone();
        changed.encrypted_outputs = [b"eo0".as_slice(), b"changed".as_slice()];
        let changed_sig = h_sig(&changed).expect("changed sig hash");
        let changed_accounting_output =
            sample_accounting_record(fields.output_commitments[1], "a651", 6, b"a651");
        let mut changed_accounting = preimage.clone();
        changed_accounting.accounting_outputs =
            [&accounting_outputs[0], &changed_accounting_output];
        let changed_accounting_sig =
            h_sig(&changed_accounting).expect("changed accounting sig hash");
        let mut changed_circuit = preimage.clone();
        changed_circuit.circuit_id = ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY;
        let changed_circuit_sig = h_sig(&changed_circuit).expect("changed circuit sig hash");

        assert_ne!(sig, changed_sig);
        assert_ne!(sig, changed_accounting_sig);
        assert_ne!(sig, changed_circuit_sig);
        assert_eq!(sig.len(), ASSET_ORCHARD_SIGHASH_BYTES);
    }

    #[test]
    fn asset_orchard_spend_authorizations_verify_and_bind_h_sig() {
        let action = signed_sample_action();
        action
            .verify_spend_authorizations("postfiat-wan-devnet", [1u8; 32], 2)
            .expect("valid spend auth");

        let mut changed = action.clone();
        changed.encrypted_outputs[1] = AssetOrchardBoundedBytes::from_bytes(
            b"changed-ciphertext",
            ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
        )
        .expect("changed output");
        let fields = changed
            .public_fields_without_binding_check()
            .expect("changed fields");
        changed.swap_binding_hash =
            AssetOrchardSwapBindingHash::from_bytes(&swap_binding_hash(&fields).expect("binding"));
        assert!(changed
            .verify_spend_authorizations("postfiat-wan-devnet", [1u8; 32], 2)
            .is_err());

        assert!(action
            .verify_spend_authorizations("wrong-chain", [1u8; 32], 2)
            .is_err());
    }

    #[test]
    fn disclosed_egress_authorization_verifies_and_binds_recipient() {
        let chain_id = "postfiat-wan-devnet";
        let genesis_hash = [3u8; 32];
        let protocol_version = 2;
        let asset_id = "ab".repeat(32);
        let note = build_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            protocol_version,
            &asset_id,
            42,
            &"11".repeat(32),
        )
        .expect("wallet note");
        let auth = build_asset_orchard_disclosed_egress_authorization(
            chain_id,
            genesis_hash,
            protocol_version,
            "buyer-account",
            &note,
        )
        .expect("egress auth");
        let pool_domain =
            AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)
                .expect("pool domain");
        let check = AssetOrchardDisclosedEgressCheck {
            preimage: AssetOrchardDisclosedEgressPreimage {
                chain_id,
                genesis_hash,
                protocol_version,
                pool_id: &note.pool_id,
                pool_domain,
                to: "buyer-account",
                asset_id: &note.asset_id,
                amount: note.value,
                output_commitment: note.output_commitment.to_field().expect("cmx"),
                nullifier: auth.nullifier.to_field().expect("nf"),
                spend_auth_verification_key: auth
                    .spend_auth_verification_key
                    .to_affine()
                    .expect("ak"),
                spend_auth_randomizer: asset_orchard_scalar_from_hex(&auth.spend_auth_randomizer)
                    .expect("alpha"),
                randomized_verification_key: auth
                    .randomized_verification_key
                    .to_affine()
                    .expect("rk"),
            },
            note: &note.note,
            nk: &note.nk,
            rivk: note.rivk.as_str(),
            spend_authorization_signature: &auth.spend_authorization_signature,
        };
        verify_asset_orchard_disclosed_egress(&check).expect("valid egress");

        let tampered = AssetOrchardDisclosedEgressCheck {
            preimage: AssetOrchardDisclosedEgressPreimage {
                to: "other-account",
                ..check.preimage.clone()
            },
            ..check
        };
        assert_eq!(
            verify_asset_orchard_disclosed_egress(&tampered)
                .expect_err("recipient change must fail")
                .code(),
            "asset_orchard_egress_spend_authorization_failed"
        );
    }

    #[test]
    fn disclosed_egress_randomizer_uses_fresh_randomness() {
        let commitment =
            AssetOrchardFieldElement::from_field(hash_to_pallas_base("test", b"cmx").expect("cmx"));
        let first = asset_orchard_egress_randomizer(&commitment, "buyer-account", "a651", 42)
            .expect("first alpha");
        let second = asset_orchard_egress_randomizer(&commitment, "buyer-account", "a651", 42)
            .expect("second alpha");
        assert_ne!(first, second);
    }

    #[test]
    fn asset_orchard_indexing_helpers_reject_count_mismatch_without_panic() {
        fn assert_count_mismatch<T>(result: std::thread::Result<Result<T, AssetOrchardError>>) {
            assert!(result.is_ok(), "malformed action must not panic");
            let error = match result.expect("catch_unwind result") {
                Ok(_) => panic!("malformed action must be rejected"),
                Err(error) => error,
            };
            assert_eq!(error.code(), "action_count_mismatch");
        }

        let mut missing_output = sample_action();
        missing_output.encrypted_outputs.pop();
        assert_count_mismatch(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || missing_output.public_fields_without_binding_check(),
        )));

        let mut missing_key = sample_action();
        missing_key.randomized_verification_keys.pop();
        assert_count_mismatch(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || missing_key.sighash("postfiat-wan-devnet", [1u8; 32], 2),
        )));

        let mut missing_signature = signed_sample_action();
        missing_signature.spend_authorization_signatures.pop();
        assert_count_mismatch(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || missing_signature.verify_spend_authorizations("postfiat-wan-devnet", [1u8; 32], 2),
        )));
    }

    #[test]
    fn asset_orchard_action_validates_and_constructs_public_instance() {
        let action = sample_action();
        action.validate().expect("valid action");
        action
            .validate_domain_binding("postfiat-wan-devnet", [1u8; 32], 2)
            .expect("domain binding");
        let instance = action.public_instance().expect("public instance");
        let fields = action.public_fields().expect("public fields");

        assert_eq!(instance.len(), ASSET_ORCHARD_PUBLIC_INSTANCE_LEN);
        assert_eq!(instance[0], fields.pool_domain);
        assert_eq!(instance[1], fields.anchor);
        assert_eq!(instance[2], fields.nullifiers[0]);
        assert_eq!(instance[8], fields.output_commitments[0]);
        assert_eq!(instance[16], pallas::Base::ZERO);
        assert_eq!(
            action
                .sighash("postfiat-wan-devnet", [1u8; 32], 2)
                .expect("sighash")
                .len(),
            ASSET_ORCHARD_SIGHASH_BYTES
        );
    }

    #[test]
    fn asset_orchard_action_rejects_tampered_binding_and_wrong_domain() {
        let mut action = sample_action();
        action.swap_binding_hash = AssetOrchardSwapBindingHash::parse_hex(
            "aa".repeat(ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES),
        )
        .expect("bad binding");
        assert_eq!(
            action.validate().expect_err("binding mismatch").code(),
            "asset_orchard_swap_binding_mismatch"
        );

        let mut action = sample_action();
        action.pricing_claim.nav_epoch += 1;
        assert_eq!(
            action
                .validate()
                .expect_err("pricing epoch must be action-bound")
                .code(),
            "asset_orchard_swap_binding_mismatch"
        );

        let action = sample_action();
        assert_eq!(
            action
                .validate_domain_binding("postfiat-wan-devnet", [2u8; 32], 2)
                .expect_err("domain mismatch")
                .code(),
            "asset_orchard_pool_domain_mismatch"
        );
    }

    #[test]
    fn asset_orchard_action_rejects_duplicate_public_state_fields() {
        let mut action = sample_action();
        action.nullifiers[1] = action.nullifiers[0].clone();
        assert_eq!(
            action.validate().expect_err("duplicate nullifier").code(),
            "duplicate_nullifier"
        );

        let mut action = sample_action();
        action.output_commitments[1] = action.output_commitments[0].clone();
        assert_eq!(
            action.validate().expect_err("duplicate commitment").code(),
            "duplicate_output_commitment"
        );
    }

    #[test]
    fn asset_orchard_wrappers_reject_noncanonical_encodings() {
        assert_eq!(
            AssetOrchardFieldElement::parse_hex("ff".repeat(ASSET_ORCHARD_FIELD_BYTES))
                .expect_err("noncanonical field")
                .code(),
            "noncanonical_pallas_field"
        );
        assert_eq!(
            AssetOrchardFieldElement::parse_hex("AA".repeat(ASSET_ORCHARD_FIELD_BYTES))
                .expect_err("uppercase field")
                .code(),
            "noncanonical_hex"
        );
        assert_eq!(
            AssetOrchardPoint::parse_hex("00".repeat(ASSET_ORCHARD_POINT_BYTES))
                .expect_err("identity point")
                .code(),
            "identity_point"
        );
    }
}
