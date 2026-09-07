#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use base64::Engine as _;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

pub mod receipts;

pub const ARC_TESTNET_CHAIN_ID: u64 = 5_042_002;
pub const ARC_VALIDATOR_REGISTRY: &str = "0x3600000000000000000000000000000000000002";
pub const ARC_COMMIT_PREIMAGE_LEN: usize = 75;
pub const MAX_VALIDATORS: usize = 256;
pub const MAX_SIGNATURES: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenFixture {
    pub schema: String,
    pub chain_id: u64,
    pub arc_node_commit: String,
    pub block: BlockFixture,
    pub certificate: CommitCertificate,
    pub validator_set: ValidatorSetFixture,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockFixture {
    pub number: u64,
    pub hash: String,
    pub receipts_root: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommitCertificate {
    pub height: u64,
    pub round: u32,
    pub block_hash: String,
    pub signatures: Vec<CommitSignature>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommitSignature {
    pub address: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorSetFixture {
    pub queried_at_block: u64,
    pub validators: Vec<Validator>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Validator {
    pub address: String,
    pub public_key: String,
    pub voting_power: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationReport {
    pub height: u64,
    pub block_hash: [u8; 32],
    pub receipts_root: [u8; 32],
    pub signed_voting_power: u128,
    pub total_voting_power: u128,
    pub signatures_verified: usize,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConformanceError {
    #[error("unsupported fixture schema: {0}")]
    Schema(String),
    #[error("wrong Arc chain id: {0}")]
    ChainId(u64),
    #[error("fixture exceeds validator/signature bounds")]
    Bounds,
    #[error("block/certificate height mismatch")]
    HeightMismatch,
    #[error("block/certificate hash mismatch")]
    BlockHashMismatch,
    #[error("validator set must be read at certificate height - 1")]
    ValidatorSetHeightMismatch,
    #[error("invalid {field}: {reason}")]
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },
    #[error("duplicate validator address: {0}")]
    DuplicateValidator(String),
    #[error("duplicate commit signer: {0}")]
    DuplicateSigner(String),
    #[error("unknown or zero-power signer: {0}")]
    UnknownSigner(String),
    #[error("validator address does not match its Ed25519 public key: {0}")]
    ValidatorAddressMismatch(String),
    #[error("invalid Ed25519 signature for signer: {0}")]
    InvalidSignature(String),
    #[error("signed voting power does not exceed two thirds")]
    SubQuorum,
    #[error("voting power overflow")]
    VotingPowerOverflow,
}

pub fn parse_fixture(json: &str) -> Result<GoldenFixture, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn verify_fixture(fixture: &GoldenFixture) -> Result<VerificationReport, ConformanceError> {
    if fixture.schema != "arc-conformance-fixture-v1" {
        return Err(ConformanceError::Schema(fixture.schema.clone()));
    }
    if fixture.chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(ConformanceError::ChainId(fixture.chain_id));
    }
    if fixture.validator_set.validators.is_empty()
        || fixture.validator_set.validators.len() > MAX_VALIDATORS
        || fixture.certificate.signatures.is_empty()
        || fixture.certificate.signatures.len() > MAX_SIGNATURES
    {
        return Err(ConformanceError::Bounds);
    }
    if fixture.block.number != fixture.certificate.height {
        return Err(ConformanceError::HeightMismatch);
    }
    if fixture.validator_set.queried_at_block.checked_add(1) != Some(fixture.certificate.height) {
        return Err(ConformanceError::ValidatorSetHeightMismatch);
    }

    let block_hash = decode_fixed_hex::<32>(&fixture.block.hash, "block.hash")?;
    let certificate_hash =
        decode_fixed_hex::<32>(&fixture.certificate.block_hash, "certificate.block_hash")?;
    if block_hash != certificate_hash {
        return Err(ConformanceError::BlockHashMismatch);
    }
    let receipts_root =
        decode_fixed_hex::<32>(&fixture.block.receipts_root, "block.receipts_root")?;

    let mut validators = BTreeMap::new();
    let mut total_voting_power = 0u128;
    for validator in &fixture.validator_set.validators {
        let address = decode_fixed_hex::<20>(&validator.address, "validator.address")?;
        let public_key = decode_fixed_hex::<32>(&validator.public_key, "validator.public_key")?;
        let derived = address_from_public_key(&public_key);
        if address != derived {
            return Err(ConformanceError::ValidatorAddressMismatch(
                validator.address.clone(),
            ));
        }
        if validators
            .insert(address, (public_key, validator.voting_power))
            .is_some()
        {
            return Err(ConformanceError::DuplicateValidator(
                validator.address.clone(),
            ));
        }
        total_voting_power = total_voting_power
            .checked_add(u128::from(validator.voting_power))
            .ok_or(ConformanceError::VotingPowerOverflow)?;
    }

    let mut seen_signers = BTreeMap::new();
    let mut signed_voting_power = 0u128;
    for signed in &fixture.certificate.signatures {
        let address = decode_fixed_hex::<20>(&signed.address, "signature.address")?;
        if seen_signers.insert(address, ()).is_some() {
            return Err(ConformanceError::DuplicateSigner(signed.address.clone()));
        }
        let Some((public_key, voting_power)) = validators.get(&address) else {
            return Err(ConformanceError::UnknownSigner(signed.address.clone()));
        };
        if *voting_power == 0 {
            return Err(ConformanceError::UnknownSigner(signed.address.clone()));
        }
        let signature_bytes = base64::engine::general_purpose::STANDARD
            .decode(&signed.signature)
            .map_err(|_| ConformanceError::InvalidField {
                field: "signature.signature",
                reason: "invalid base64",
            })?;
        let signature = Signature::from_slice(&signature_bytes).map_err(|_| {
            ConformanceError::InvalidField {
                field: "signature.signature",
                reason: "expected 64-byte Ed25519 signature",
            }
        })?;
        let verifying_key =
            VerifyingKey::from_bytes(public_key).map_err(|_| ConformanceError::InvalidField {
                field: "validator.public_key",
                reason: "invalid Ed25519 point",
            })?;
        let preimage = commit_preimage(
            fixture.certificate.height,
            fixture.certificate.round,
            block_hash,
            address,
        );
        verifying_key
            .verify_strict(&preimage, &signature)
            .map_err(|_| ConformanceError::InvalidSignature(signed.address.clone()))?;
        signed_voting_power = signed_voting_power
            .checked_add(u128::from(*voting_power))
            .ok_or(ConformanceError::VotingPowerOverflow)?;
    }

    let signed_times_three = signed_voting_power
        .checked_mul(3)
        .ok_or(ConformanceError::VotingPowerOverflow)?;
    let total_times_two = total_voting_power
        .checked_mul(2)
        .ok_or(ConformanceError::VotingPowerOverflow)?;
    if signed_times_three <= total_times_two {
        return Err(ConformanceError::SubQuorum);
    }

    Ok(VerificationReport {
        height: fixture.certificate.height,
        block_hash,
        receipts_root,
        signed_voting_power,
        total_voting_power,
        signatures_verified: fixture.certificate.signatures.len(),
    })
}

/// Rebuild Arc v0.8.0's canonical SSZ bytes for a non-nil precommit.
///
/// The SSZ container's fixed section is 37 bytes: the one-byte precommit tag,
/// little-endian height, offsets to the variable round/value fields, and the
/// 20-byte validator address. Both variable fields are `Some`, encoded as a
/// one-byte union selector followed by their value.
pub fn commit_preimage(
    height: u64,
    round: u32,
    block_hash: [u8; 32],
    validator_address: [u8; 20],
) -> [u8; ARC_COMMIT_PREIMAGE_LEN] {
    const FIXED_SECTION_LEN: u32 = 37;
    const ROUND_SECTION_LEN: u32 = 5;

    let mut out = [0u8; ARC_COMMIT_PREIMAGE_LEN];
    out[0] = 1; // SszVoteType::Precommit
    out[1..9].copy_from_slice(&height.to_le_bytes());
    out[9..13].copy_from_slice(&FIXED_SECTION_LEN.to_le_bytes());
    out[13..17].copy_from_slice(&(FIXED_SECTION_LEN + ROUND_SECTION_LEN).to_le_bytes());
    out[17..37].copy_from_slice(&validator_address);
    out[37] = 1; // Some(round)
    out[38..42].copy_from_slice(&round.to_le_bytes());
    out[42] = 1; // Some(value_id)
    out[43..75].copy_from_slice(&block_hash);
    out
}

pub fn address_from_public_key(public_key: &[u8; 32]) -> [u8; 20] {
    let digest = Keccak256::digest(public_key);
    let mut address = [0u8; 20];
    address.copy_from_slice(&digest[..20]);
    address
}

pub fn decode_fixed_hex<const N: usize>(
    value: &str,
    field: &'static str,
) -> Result<[u8; N], ConformanceError> {
    let raw = value
        .strip_prefix("0x")
        .ok_or(ConformanceError::InvalidField {
            field,
            reason: "missing 0x prefix",
        })?;
    let bytes = hex::decode(raw).map_err(|_| ConformanceError::InvalidField {
        field,
        reason: "invalid hex",
    })?;
    bytes
        .try_into()
        .map_err(|_| ConformanceError::InvalidField {
            field,
            reason: "wrong byte length",
        })
}

pub fn encode_hex(bytes: impl AsRef<[u8]>) -> String {
    format!("0x{}", hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_A: &str = include_str!("../fixtures/arc-block-a.json");
    const FIXTURE_B: &str = include_str!("../fixtures/arc-block-b.json");

    #[test]
    fn golden_commit() {
        for raw in [FIXTURE_A, FIXTURE_B] {
            let fixture = parse_fixture(raw).expect("fixture JSON must be canonical");
            let report = verify_fixture(&fixture).expect("golden Arc commit must verify");
            assert!(report.signatures_verified > 0);
            assert!(report.signed_voting_power * 3 > report.total_voting_power * 2);
        }
    }

    #[test]
    fn every_preimage_byte_is_authenticated() {
        let fixture = parse_fixture(FIXTURE_A).expect("fixture JSON must be canonical");
        let signed = &fixture.certificate.signatures[0];
        let address = decode_fixed_hex::<20>(&signed.address, "signature.address").unwrap();
        let validator = fixture
            .validator_set
            .validators
            .iter()
            .find(|validator| validator.address == signed.address)
            .expect("signer is in validator set");
        let public_key =
            decode_fixed_hex::<32>(&validator.public_key, "validator.public_key").unwrap();
        let verifying_key = VerifyingKey::from_bytes(&public_key).unwrap();
        let signature_bytes = base64::engine::general_purpose::STANDARD
            .decode(&signed.signature)
            .unwrap();
        let signature = Signature::from_slice(&signature_bytes).unwrap();
        let block_hash = decode_fixed_hex::<32>(&fixture.block.hash, "block.hash").unwrap();
        let preimage = commit_preimage(
            fixture.certificate.height,
            fixture.certificate.round,
            block_hash,
            address,
        );

        verifying_key.verify_strict(&preimage, &signature).unwrap();
        for index in 0..preimage.len() {
            let mut mutated = preimage;
            mutated[index] ^= 1;
            assert!(
                verifying_key.verify_strict(&mutated, &signature).is_err(),
                "mutation at byte {index} unexpectedly verified"
            );
        }
    }

    #[test]
    fn binding_and_quorum_fail_closed() {
        let fixture = parse_fixture(FIXTURE_A).expect("fixture JSON must be canonical");

        let mut wrong_hash = fixture.clone();
        wrong_hash.block.hash = format!("0x{}", "00".repeat(32));
        assert_eq!(
            verify_fixture(&wrong_hash),
            Err(ConformanceError::BlockHashMismatch)
        );

        let mut sub_quorum = fixture;
        sub_quorum.certificate.signatures.truncate(1);
        assert_eq!(
            verify_fixture(&sub_quorum),
            Err(ConformanceError::SubQuorum)
        );
    }
}
