//! Step 6 of the deposit proof: Alpenglow finality.
//!
//! A block is final when validators holding enough stake signed for it. Two forms are accepted
//! (`votor-messages/src/certificate.rs`):
//!
//! - `FinalizeFast(slot, block_id)`: at least 80% of stake signed a notarization vote for the block.
//! - `Finalize(slot)` with `Notarize(slot, block_id)`: at least 60% each. A `Finalize` certificate
//!   names only the slot, so it is never accepted without the notarization that names the block.
//!
//! Fallback votes are not accepted. They would need the two-payload bitmap and are not required
//! to show finality.
//!
//! The signature scheme is BLS12-381 with public keys in G1 and signatures in G2, hashed to the
//! curve under `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_` (`solana-bls-signatures`, `hash.rs`).
//! Aggregation is plain addition, which is sound only if every key was registered with a proof
//! of possession; the caller is responsible for the validator set being authenticated.

use bls12_381::{
    hash_to_curve::{ExpandMsgXmd, HashToCurve},
    multi_miller_loop, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective, Gt,
};
use sha2::{Digest, Sha256};

pub const PUBKEY_LEN: usize = 48;
/// Certificates carry the aggregate signature as an uncompressed G2 point (`solana-bls-signatures`, `Signature`).
pub const SIGNATURE_LEN: usize = 192;
const HASH_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";
const SET_HASH_DOMAIN: &[u8] = b"postfiat.pfusdc.solana.validator_set.v1";
const BITMAP_BASE2: u8 = 0;
const VOTE_NOTARIZE: u8 = 1;
const VOTE_FINALIZE: u8 = 2;
/// SIMD-0326 caps the voting set at the 2,000 highest-staked validators.
pub const MAX_VALIDATORS: usize = 2000;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CertificateError {
    EmptyValidatorSet,
    TooManyValidators,
    ZeroStake,
    StakeOverflow,
    UnsupportedBitmap,
    BitmapLength,
    BitmapPadding,
    NoSigners,
    InvalidPublicKey,
    InvalidSignature,
    SignatureMismatch,
    NotEnoughStake,
}

/// One validator in rank order: highest stake first (`runtime/src/epoch_stakes.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validator {
    pub pubkey: [u8; PUBKEY_LEN],
    pub stake: u64,
}

/// The validator set a certificate is checked against, with the stake thresholds are measured against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorSet {
    pub validators: Vec<Validator>,
    /// At least the sum of the listed stakes. The bridge uses the runtime's total epoch stake,
    /// which is never smaller than Alpenglow's own denominator, so omitting validators from the
    /// list can only make a certificate harder to accept.
    pub total_stake: u128,
}

impl ValidatorSet {
    pub fn new(validators: Vec<Validator>, total_stake: u128) -> Result<Self, CertificateError> {
        if validators.is_empty() {
            return Err(CertificateError::EmptyValidatorSet);
        }
        if validators.len() > MAX_VALIDATORS {
            return Err(CertificateError::TooManyValidators);
        }
        let mut listed: u128 = 0;
        for validator in &validators {
            if validator.stake == 0 {
                return Err(CertificateError::ZeroStake);
            }
            listed += u128::from(validator.stake);
        }
        if total_stake < listed {
            return Err(CertificateError::StakeOverflow);
        }
        Ok(Self {
            validators,
            total_stake,
        })
    }

    /// Commits to every key, every stake, their order and the denominator.
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(SET_HASH_DOMAIN);
        hasher.update(self.total_stake.to_le_bytes());
        hasher.update((self.validators.len() as u32).to_le_bytes());
        for validator in &self.validators {
            hasher.update(validator.pubkey);
            hasher.update(validator.stake.to_le_bytes());
        }
        hasher.finalize().into()
    }
}

/// Whether 48 bytes are a compressed BLS12-381 public key this program will accept: a point of
/// the prime-order subgroup of G1 other than the identity.
pub fn valid_pubkey(pubkey: &[u8; PUBKEY_LEN]) -> bool {
    Option::<G1Affine>::from(G1Affine::from_compressed(pubkey))
        .map(|point| !bool::from(point.is_identity()))
        .unwrap_or(false)
}

/// The bytes a validator signs for a notarization vote: `tag 1 | slot | block_id | shred_version`
/// (`votor-messages/src/wire.rs`, `VotePayloadToSign::Notar`).
pub fn notarize_payload(slot: u64, block_id: &[u8; 32], shred_version: u16) -> [u8; 43] {
    let mut out = [0u8; 43];
    out[0] = VOTE_NOTARIZE;
    out[1..9].copy_from_slice(&slot.to_le_bytes());
    out[9..41].copy_from_slice(block_id);
    out[41..43].copy_from_slice(&shred_version.to_le_bytes());
    out
}

/// The bytes signed for a finalization vote: `tag 2 | slot | shred_version`.
pub fn finalize_payload(slot: u64, shred_version: u16) -> [u8; 11] {
    let mut out = [0u8; 11];
    out[0] = VOTE_FINALIZE;
    out[1..9].copy_from_slice(&slot.to_le_bytes());
    out[9..11].copy_from_slice(&shred_version.to_le_bytes());
    out
}

/// Ranks set in a single-payload bitmap: `version 0 | bit count u16 | bits, least significant first`
/// (`solana-signer-store`).
pub(crate) fn signer_ranks(
    bitmap: &[u8],
    validators: usize,
) -> Result<Vec<usize>, CertificateError> {
    if bitmap.len() < 3 || bitmap[0] != BITMAP_BASE2 {
        return Err(CertificateError::UnsupportedBitmap);
    }
    let bits = usize::from(u16::from_le_bytes([bitmap[1], bitmap[2]]));
    let data = &bitmap[3..];
    if bits > validators || data.len() != bits.div_ceil(8) {
        return Err(CertificateError::BitmapLength);
    }
    let mut ranks = Vec::new();
    for (byte_index, byte) in data.iter().enumerate() {
        for bit in 0..8 {
            if byte >> bit & 1 == 1 {
                let rank = byte_index * 8 + bit;
                if rank >= bits {
                    return Err(CertificateError::BitmapPadding);
                }
                ranks.push(rank);
            }
        }
    }
    if ranks.is_empty() {
        return Err(CertificateError::NoSigners);
    }
    Ok(ranks)
}

/// One aggregate signature over one payload by the validators a bitmap names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregate<'a> {
    pub signature: &'a [u8; SIGNATURE_LEN],
    pub bitmap: &'a [u8],
}

/// Checks the aggregate over `payload` and returns the signers' ranks and their combined stake.
pub fn verify_aggregate(
    set: &ValidatorSet,
    aggregate: &Aggregate,
    payload: &[u8],
) -> Result<(Vec<usize>, u128), CertificateError> {
    let ranks = signer_ranks(aggregate.bitmap, set.validators.len())?;
    let mut key = G1Projective::identity();
    let mut stake: u128 = 0;
    for &rank in &ranks {
        let validator = &set.validators[rank];
        let point = Option::<G1Affine>::from(G1Affine::from_compressed(&validator.pubkey))
            .ok_or(CertificateError::InvalidPublicKey)?;
        if bool::from(point.is_identity()) {
            return Err(CertificateError::InvalidPublicKey);
        }
        key += point;
        stake += u128::from(validator.stake);
    }
    let signature = Option::<G2Affine>::from(G2Affine::from_uncompressed(aggregate.signature))
        .ok_or(CertificateError::InvalidSignature)?;
    if bool::from(signature.is_identity()) || bool::from(key.is_identity()) {
        return Err(CertificateError::InvalidSignature);
    }
    let message = G2Affine::from(<G2Projective as HashToCurve<
        ExpandMsgXmd<sha2_v09::Sha256>,
    >>::hash_to_curve(payload, HASH_DST));
    // e(key, H(m)) == e(g1, signature), as one product: e(key, H(m)) * e(-g1, signature) == 1.
    let key = G1Affine::from(key);
    let generator = -G1Affine::generator();
    let product = multi_miller_loop(&[
        (&key, &G2Prepared::from(message)),
        (&generator, &G2Prepared::from(signature)),
    ])
    .final_exponentiation();
    if product != Gt::identity() {
        return Err(CertificateError::SignatureMismatch);
    }
    Ok((ranks, stake))
}

fn meets(stake: u128, total: u128, percent: u128) -> bool {
    stake >= (total / 100) * percent + ((total % 100) * percent).div_ceil(100)
}

/// How Solana finalized the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finality<'a> {
    /// One certificate from notarization votes holding at least 80% of stake.
    Fast(Aggregate<'a>),
    /// A notarization certificate for the block and a finalization certificate for its slot, at
    /// least 60% each.
    Slow {
        notarize: Aggregate<'a>,
        finalize: Aggregate<'a>,
    },
}

/// Returns the ranks whose signatures establish finality of `(slot, block_id)`: for the fast
/// form the notarization signers, for the slow form the finalization signers.
pub fn verify_finality(
    set: &ValidatorSet,
    slot: u64,
    block_id: &[u8; 32],
    shred_version: u16,
    finality: &Finality,
) -> Result<Vec<usize>, CertificateError> {
    let notar = notarize_payload(slot, block_id, shred_version);
    match finality {
        Finality::Fast(aggregate) => {
            let (ranks, stake) = verify_aggregate(set, aggregate, &notar)?;
            if !meets(stake, set.total_stake, 80) {
                return Err(CertificateError::NotEnoughStake);
            }
            Ok(ranks)
        }
        Finality::Slow { notarize, finalize } => {
            let (_, notar_stake) = verify_aggregate(set, notarize, &notar)?;
            if !meets(notar_stake, set.total_stake, 60) {
                return Err(CertificateError::NotEnoughStake);
            }
            let (ranks, final_stake) =
                verify_aggregate(set, finalize, &finalize_payload(slot, shred_version))?;
            if !meets(final_stake, set.total_stake, 60) {
                return Err(CertificateError::NotEnoughStake);
            }
            Ok(ranks)
        }
    }
}
