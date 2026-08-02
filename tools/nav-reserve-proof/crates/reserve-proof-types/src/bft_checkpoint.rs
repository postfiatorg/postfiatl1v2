//! Provider-neutral external-chain checkpoints certified by a governed PFTL
//! validator committee.

use alloy_primitives::B256;
use postfiat_crypto_provider::{
    ml_dsa_65_verify_with_context, ML_DSA_65_PUBLIC_KEY_BYTES, ML_DSA_65_SIGNATURE_BYTES,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};

pub const BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1: &[u8] =
    b"postfiat-l1-v2/reserve-source-checkpoint/v1";
pub const MAX_BFT_CHECKPOINT_VALIDATORS: usize = 64;

const COMMITTEE_ROOT_DOMAIN: &[u8] = b"postfiat.reserve_source_checkpoint_committee.v1";
const CHECKPOINT_DOMAIN: &[u8] = b"postfiat.reserve_source_checkpoint.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BftCheckpointValidatorV1 {
    pub validator_id: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BftCheckpointCommitteeV1 {
    pub epoch: u64,
    pub quorum: u16,
    pub validators: Vec<BftCheckpointValidatorV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BftSourceCheckpointV1 {
    pub pftl_genesis_hash: String,
    pub checkpoint_kind: String,
    pub source_domain: String,
    pub source_height: u64,
    pub source_block_hash: B256,
    pub source_state_commitment: B256,
    pub observed_source_head: u64,
    pub minimum_depth: u32,
    pub pftl_observation_height: u64,
    pub committee_epoch: u64,
    pub committee_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BftSourceCheckpointVoteV1 {
    pub validator_id: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BftSourceCheckpointCertificateV1 {
    pub committee: BftCheckpointCommitteeV1,
    pub checkpoint: BftSourceCheckpointV1,
    pub votes: Vec<BftSourceCheckpointVoteV1>,
}

impl BftCheckpointCommitteeV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.epoch == 0
            || self.validators.is_empty()
            || self.validators.len() > MAX_BFT_CHECKPOINT_VALIDATORS
            || self.quorum == 0
            || usize::from(self.quorum) > self.validators.len()
        {
            return Err("source checkpoint committee bounds are invalid".to_string());
        }
        let mut previous = None;
        for validator in &self.validators {
            validate_identifier("source checkpoint validator", &validator.validator_id)?;
            if validator.public_key.len() != ML_DSA_65_PUBLIC_KEY_BYTES {
                return Err("source checkpoint validator key length is invalid".to_string());
            }
            if previous >= Some(validator.validator_id.as_str()) {
                return Err(
                    "source checkpoint validators must be strictly sorted and unique".to_string(),
                );
            }
            previous = Some(validator.validator_id.as_str());
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.epoch.to_be_bytes());
        bytes.extend_from_slice(&self.quorum.to_be_bytes());
        append_u32(&mut bytes, self.validators.len())?;
        for validator in &self.validators {
            append_bytes(&mut bytes, validator.validator_id.as_bytes())?;
            append_bytes(&mut bytes, &validator.public_key)?;
        }
        Ok(hash48(COMMITTEE_ROOT_DOMAIN, &[&bytes]))
    }
}

impl BftSourceCheckpointV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        validate_hex("checkpoint PFTL genesis", &self.pftl_genesis_hash, 48)?;
        validate_identifier("checkpoint kind", &self.checkpoint_kind)?;
        validate_identifier("checkpoint source domain", &self.source_domain)?;
        validate_hex("checkpoint committee root", &self.committee_root, 48)?;
        if self.source_height == 0
            || self.source_block_hash == B256::ZERO
            || self.source_state_commitment == B256::ZERO
            || self.observed_source_head == 0
            || self.minimum_depth == 0
            || self.pftl_observation_height == 0
            || self.committee_epoch == 0
        {
            return Err("source checkpoint contains a zero required field".to_string());
        }
        let required_head = self
            .source_height
            .checked_add(u64::from(self.minimum_depth))
            .ok_or_else(|| "source checkpoint depth overflows".to_string())?;
        if self.observed_source_head < required_head {
            return Err("source checkpoint is below its governed depth".to_string());
        }
        let mut bytes = Vec::new();
        append_hex(&mut bytes, &self.pftl_genesis_hash, 48)?;
        append_bytes(&mut bytes, self.checkpoint_kind.as_bytes())?;
        append_bytes(&mut bytes, self.source_domain.as_bytes())?;
        bytes.extend_from_slice(&self.source_height.to_be_bytes());
        bytes.extend_from_slice(self.source_block_hash.as_slice());
        bytes.extend_from_slice(self.source_state_commitment.as_slice());
        bytes.extend_from_slice(&self.observed_source_head.to_be_bytes());
        bytes.extend_from_slice(&self.minimum_depth.to_be_bytes());
        bytes.extend_from_slice(&self.pftl_observation_height.to_be_bytes());
        bytes.extend_from_slice(&self.committee_epoch.to_be_bytes());
        append_hex(&mut bytes, &self.committee_root, 48)?;
        Ok(domain_message(CHECKPOINT_DOMAIN, &bytes))
    }

    pub fn vote_signing_statement(&self, validator_id: &str) -> Result<Vec<u8>, String> {
        validate_identifier("source checkpoint vote validator", validator_id)?;
        let mut statement = self.canonical_bytes()?;
        append_bytes(&mut statement, validator_id.as_bytes())?;
        Ok(statement)
    }
}

impl BftSourceCheckpointCertificateV1 {
    pub fn verify(&self) -> Result<(), String> {
        self.committee.validate()?;
        let committee_root = self.committee.root()?;
        if self.checkpoint.committee_epoch != self.committee.epoch
            || self.checkpoint.committee_root != committee_root
        {
            return Err("source checkpoint committee binding mismatch".to_string());
        }
        self.checkpoint.canonical_bytes()?;
        if self.votes.len() < usize::from(self.committee.quorum)
            || self.votes.len() > self.committee.validators.len()
        {
            return Err("source checkpoint vote count is out of bounds".to_string());
        }
        let mut previous = None;
        for vote in &self.votes {
            validate_identifier("source checkpoint vote validator", &vote.validator_id)?;
            if vote.signature.len() != ML_DSA_65_SIGNATURE_BYTES {
                return Err("source checkpoint signature length is invalid".to_string());
            }
            if previous >= Some(vote.validator_id.as_str()) {
                return Err(
                    "source checkpoint votes must be strictly sorted and unique".to_string()
                );
            }
            previous = Some(vote.validator_id.as_str());
            let validator = self
                .committee
                .validators
                .iter()
                .find(|validator| validator.validator_id == vote.validator_id)
                .ok_or_else(|| "source checkpoint vote is from an unknown validator".to_string())?;
            let statement = self.checkpoint.vote_signing_statement(&vote.validator_id)?;
            if !ml_dsa_65_verify_with_context(
                &validator.public_key,
                &statement,
                &vote.signature,
                BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
            ) {
                return Err("source checkpoint signature is invalid".to_string());
            }
        }
        Ok(())
    }
}

fn validate_identifier(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(format!("{field} must be bounded canonical lowercase ASCII"));
    }
    Ok(())
}

fn validate_hex(field: &str, value: &str, bytes: usize) -> Result<(), String> {
    if value.len() != bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be canonical lowercase hex"));
    }
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), String> {
    validate_hex("source checkpoint hex", value, bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| "checkpoint hex decode failed")?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let length = u32::try_from(value.len()).map_err(|_| "checkpoint field too long")?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "checkpoint count overflows")?;
    out.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn domain_message(domain: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(domain.len().saturating_add(payload.len()).saturating_add(8));
    out.extend_from_slice(&(domain.len() as u32).to_be_bytes());
    out.extend_from_slice(domain);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn hash48(domain: &[u8], parts: &[&[u8]]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update((domain.len() as u32).to_be_bytes());
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    fn fixture() -> (BftSourceCheckpointCertificateV1, Vec<MlDsa65KeyPair>) {
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        let committee = BftCheckpointCommitteeV1 {
            epoch: 9,
            quorum: 3,
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| BftCheckpointValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: "external-head-v1".to_string(),
            source_domain: "example:mainnet".to_string(),
            source_height: 1_000,
            source_block_hash: B256::repeat_byte(0x22),
            source_state_commitment: B256::repeat_byte(0x33),
            observed_source_head: 1_012,
            minimum_depth: 12,
            pftl_observation_height: 500,
            committee_epoch: committee.epoch,
            committee_root: committee.root().unwrap(),
        };
        let mut certificate = BftSourceCheckpointCertificateV1 {
            committee,
            checkpoint,
            votes: (0..3)
                .map(|index| BftSourceCheckpointVoteV1 {
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                })
                .collect(),
        };
        for (index, vote) in certificate.votes.iter_mut().enumerate() {
            let statement = certificate
                .checkpoint
                .vote_signing_statement(&vote.validator_id)
                .unwrap();
            vote.signature = ml_dsa_65_sign_with_context_seed(
                &keys[index].private_key,
                &statement,
                BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                &[0xa0 + index as u8; 32],
            )
            .unwrap();
        }
        (certificate, keys)
    }

    #[test]
    fn verifies_quorum_certified_external_checkpoint() {
        let (certificate, _) = fixture();
        certificate.verify().unwrap();
    }

    #[test]
    fn rejects_duplicate_unknown_invalid_and_foreign_votes() {
        let (certificate, _) = fixture();

        let mut duplicate = certificate.clone();
        duplicate.votes[1].validator_id = duplicate.votes[0].validator_id.clone();
        assert!(duplicate.verify().is_err());

        let mut invalid = certificate.clone();
        invalid.votes[0].signature[0] ^= 1;
        assert!(invalid.verify().is_err());

        let mut unknown = certificate.clone();
        unknown.votes[0].validator_id = "validator-unknown".to_string();
        assert!(unknown.verify().is_err());

        let mut foreign = certificate;
        foreign.checkpoint.source_domain = "other:mainnet".to_string();
        assert!(foreign.verify().is_err());
    }

    #[test]
    fn rejects_committee_root_depth_and_bound_substitution() {
        let (certificate, _) = fixture();

        let mut bad_root = certificate.clone();
        bad_root.checkpoint.committee_root = "99".repeat(48);
        assert!(bad_root.verify().is_err());

        let mut shallow = certificate.clone();
        shallow.checkpoint.observed_source_head = 1_011;
        assert!(shallow.verify().is_err());

        let mut oversized = certificate;
        oversized.committee.validators =
            vec![oversized.committee.validators[0].clone(); MAX_BFT_CHECKPOINT_VALIDATORS + 1];
        assert!(oversized.verify().is_err());
    }
}
