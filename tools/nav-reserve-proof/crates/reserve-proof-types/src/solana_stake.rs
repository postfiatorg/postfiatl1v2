//! Public, provider-neutral verification of the governed Solana stake
//! attestation format.
//!
//! This adapter preserves the historical trust boundary: quantity is
//! attested, not cryptographic. It makes the attestors, exact stake-account
//! set, ownership, parser, slot, and stake state publicly auditable.

use alloy_primitives::{keccak256, B256};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sha3::{Digest as Sha3Digest, Sha3_384};

use crate::bft_checkpoint::BftSourceCheckpointCertificateV1;

pub const SOLANA_STAKE_ADAPTER_KIND_V1: &str = "solana-stake-attested-state-v1";
pub const SOLANA_STAKE_READER_ADAPTER_KIND_V1: &str = "solana-stake-reader-bft-checkpoint-v1";
pub const SOLANA_STAKE_READER_CHECKPOINT_KIND_V1: &str = "solana-stake-reader-receipt-v1";
pub const SOLANA_DEACTIVATION_EPOCH_DISABLED: u64 = u64::MAX;
pub const STAKE_STATE_V2_DELEGATED: u32 = 2;
pub const CLOCK_EPOCH_OFFSET: usize = 16;
pub const CLOCK_MIN_LEN: usize = 40;
pub const MAX_SOLANA_ATTESTORS: usize = 16;
pub const MAX_SOLANA_STAKE_ACCOUNTS: usize = 64;
pub const MAX_SOLANA_ACCOUNT_DATA_BYTES: usize = 4096;
pub const MAX_SOLANA_TOTAL_DATA_BYTES: usize = 256 * 1024;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_solana_stake_policy.v1";
const OWNER_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_solana_stake_owner.v1";
const OWNER_AUTHORIZATION_DOMAIN: &[u8] = b"postfiat.reserve_solana_stake_owner_authorization.v1";
const ATTESTATION_DOMAIN: &[u8] = b"postfiat.reserve_solana_stake_attestation.v1";
const EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_solana_stake_evidence.v1";
const READER_POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_solana_reader_policy.v1";
const READER_STATE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_solana_reader_state.v1";
const READER_OWNER_AUTHORIZATION_DOMAIN: &[u8] =
    b"postfiat.reserve_solana_reader_owner_authorization.v1";
const READER_EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_solana_reader_evidence.v1";
const READER_INSTRUCTION_MAGIC: &[u8; 8] = b"PFSOL001";
const READER_SNAPSHOT_MAGIC: &[u8; 8] = b"PFSNAP01";
const READER_SNAPSHOT_VERSION: u16 = 1;
const SOLANA_CLOCK_SYSVAR_ID: &str = "SysvarC1ock11111111111111111111111111111111";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SolanaAttestorKindV1 {
    SelfAttested,
    OwnNode,
    PublicRpc,
}

impl SolanaAttestorKindV1 {
    fn tag(self) -> u8 {
        match self {
            Self::SelfAttested => 1,
            Self::OwnNode => 2,
            Self::PublicRpc => 3,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaAttestorPolicyV1 {
    pub source_id: String,
    pub kind: SolanaAttestorKindV1,
    pub public_key: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakePositionPolicyV1 {
    pub index: u32,
    pub address: String,
    pub vote_account: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakePolicyV1 {
    pub source_domain: String,
    pub position_set_id: String,
    pub stake_program: String,
    pub stake_seed_prefix: String,
    pub wallet: String,
    pub wallet_pubkey: [u8; 32],
    pub stake_authority: [u8; 32],
    pub withdraw_authority: [u8; 32],
    pub allow_self_attested: bool,
    pub minimum_own_nodes: u16,
    pub minimum_public_rpcs: u16,
    pub attestors: Vec<SolanaAttestorPolicyV1>,
    pub positions: Vec<SolanaStakePositionPolicyV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakeAccountSnapshotV1 {
    pub index: u32,
    pub address: String,
    pub exists: bool,
    pub lamports: u64,
    pub owner_program: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakeAttestationV1 {
    pub source_id: String,
    pub kind: SolanaAttestorKindV1,
    pub signer_public_key: [u8; 32],
    pub signature: Vec<u8>,
    pub finalized_slot: u64,
    pub clock_sysvar_data: Vec<u8>,
    pub stake_accounts: Vec<SolanaStakeAccountSnapshotV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakeAttestedProofV1 {
    pub policy: SolanaStakePolicyV1,
    pub ownership_signature: Vec<u8>,
    pub attestations: Vec<SolanaStakeAttestationV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakeReaderPolicyV1 {
    pub source_domain: String,
    pub position_set_id: String,
    pub stake_program: String,
    pub reader_program: String,
    pub reader_program_data: String,
    pub reader_program_data_hash: [u8; 32],
    pub wallet: String,
    pub wallet_pubkey: [u8; 32],
    pub stake_authority: [u8; 32],
    pub withdraw_authority: [u8; 32],
    pub positions: Vec<SolanaStakePositionPolicyV1>,
    pub checkpoint_committee_root: String,
    /// Minimum number of finalized slots which must follow the reader
    /// transaction before it can be certified.
    pub minimum_finalized_depth: u32,
    /// Maximum distance between the reader transaction and the finalized head
    /// observed by the checkpoint committee.
    pub maximum_finalized_slot_lag: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakeReaderProofV1 {
    pub policy: SolanaStakeReaderPolicyV1,
    pub checkpoint_certificate: BftSourceCheckpointCertificateV1,
    pub ownership_signature: Vec<u8>,
    pub transaction_signature: Vec<u8>,
    pub transaction_message: Vec<u8>,
    pub instruction_salt: [u8; 32],
    pub reader_payload: Vec<u8>,
    pub reader_return_data_hash: [u8; 32],
    pub reader_slot: u64,
    pub reader_epoch: u64,
    pub positions: Vec<SolanaStakeReaderPositionV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolanaStakeReaderPositionV1 {
    pub index: u32,
    pub address: String,
    pub lamports: u64,
    pub owner_program: String,
    pub data_hash: [u8; 32],
    pub stake_authority: [u8; 32],
    pub withdraw_authority: [u8; 32],
    pub vote_account: [u8; 32],
    pub delegated_lamports: u64,
    pub activation_epoch: u64,
    pub deactivation_epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SolanaParsedStakePositionV1 {
    pub index: u32,
    pub address: String,
    pub total_lamports: u64,
    pub delegated_lamports: u64,
    pub activation_epoch: u64,
    pub deactivation_epoch: u64,
    pub locked_lamports: u64,
    pub liquid_lamports: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SolanaStakeVerificationV1 {
    pub finalized_slot: u64,
    pub current_epoch: u64,
    pub total_lamports: u64,
    pub locked_lamports: u64,
    pub liquid_lamports: u64,
    pub positions: Vec<SolanaParsedStakePositionV1>,
    pub evidence_commitment: String,
}

pub struct SolanaStakeVerifyContextV1<'a> {
    pub pftl_genesis_hash: &'a str,
    pub nav_asset_id: &'a str,
    pub proof_profile_id: &'a str,
    pub valuation_policy_hash: &'a str,
    pub source_manifest_hash: &'a str,
    pub source_id: &'a str,
    pub source_domain: &'a str,
    pub asset_or_position_id: &'a str,
    pub reserve_owner_commitment: &'a str,
    pub quantity_verifier_commitment: &'a str,
    pub observed_at_pftl_height: u64,
    pub expected_evidence_commitment: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolanaStakeError {
    BoundsExceeded,
    PolicyMismatch,
    OwnerAuthorization,
    AttestorMismatch,
    AttestationSignature,
    AttestationDisagreement,
    InsufficientAttestations,
    PositionMismatch,
    StakeState,
    Clock,
    EvidenceCommitment,
    ArithmeticOverflow,
    CheckpointMismatch,
    ReaderMismatch,
    Transaction,
}

impl SolanaStakePolicyV1 {
    pub fn validate(&self) -> Result<(), SolanaStakeError> {
        validate_identifier(&self.source_domain)?;
        validate_identifier(&self.position_set_id)?;
        validate_text(&self.stake_program)?;
        validate_text(&self.stake_seed_prefix)?;
        validate_text(&self.wallet)?;
        if decode_pubkey(&self.wallet)? != self.wallet_pubkey
            || self.wallet_pubkey == [0; 32]
            || self.stake_authority == [0; 32]
            || self.withdraw_authority == [0; 32]
            || self.attestors.is_empty()
            || self.attestors.len() > MAX_SOLANA_ATTESTORS
            || self.positions.is_empty()
            || self.positions.len() > MAX_SOLANA_STAKE_ACCOUNTS
        {
            return Err(SolanaStakeError::PolicyMismatch);
        }
        let mut previous_attestor: Option<&str> = None;
        let mut own = 0usize;
        let mut public = 0usize;
        let mut self_attested = 0usize;
        let mut keys = Vec::with_capacity(self.attestors.len());
        for attestor in &self.attestors {
            validate_identifier(&attestor.source_id)?;
            if previous_attestor >= Some(attestor.source_id.as_str())
                || attestor.public_key == [0; 32]
                || keys.contains(&attestor.public_key)
            {
                return Err(SolanaStakeError::PolicyMismatch);
            }
            previous_attestor = Some(attestor.source_id.as_str());
            keys.push(attestor.public_key);
            match attestor.kind {
                SolanaAttestorKindV1::SelfAttested => self_attested += 1,
                SolanaAttestorKindV1::OwnNode => own += 1,
                SolanaAttestorKindV1::PublicRpc => public += 1,
            }
        }
        if (!self.allow_self_attested && self_attested != 0)
            || own < usize::from(self.minimum_own_nodes)
            || public < usize::from(self.minimum_public_rpcs)
        {
            return Err(SolanaStakeError::InsufficientAttestations);
        }
        let mut previous_index = None;
        let mut previous_address: Option<&str> = None;
        for position in &self.positions {
            validate_text(&position.address)?;
            if previous_index >= Some(position.index)
                || previous_address >= Some(position.address.as_str())
                || position.vote_account == [0; 32]
                || derive_stake_address(
                    self.wallet_pubkey,
                    &self.stake_seed_prefix,
                    position.index,
                    &self.stake_program,
                )? != position.address
            {
                return Err(SolanaStakeError::PolicyMismatch);
            }
            previous_index = Some(position.index);
            previous_address = Some(position.address.as_str());
        }
        Ok(())
    }

    pub fn commitment(&self) -> Result<String, SolanaStakeError> {
        self.validate()?;
        let mut out = Vec::new();
        for value in [
            self.source_domain.as_bytes(),
            self.position_set_id.as_bytes(),
            self.stake_program.as_bytes(),
            self.stake_seed_prefix.as_bytes(),
            self.wallet.as_bytes(),
        ] {
            append_bytes(&mut out, value)?;
        }
        out.extend_from_slice(&self.wallet_pubkey);
        out.extend_from_slice(&self.stake_authority);
        out.extend_from_slice(&self.withdraw_authority);
        out.push(u8::from(self.allow_self_attested));
        out.extend_from_slice(&self.minimum_own_nodes.to_be_bytes());
        out.extend_from_slice(&self.minimum_public_rpcs.to_be_bytes());
        append_u32(&mut out, self.attestors.len())?;
        for attestor in &self.attestors {
            append_bytes(&mut out, attestor.source_id.as_bytes())?;
            out.push(attestor.kind.tag());
            out.extend_from_slice(&attestor.public_key);
        }
        append_u32(&mut out, self.positions.len())?;
        for position in &self.positions {
            out.extend_from_slice(&position.index.to_be_bytes());
            append_bytes(&mut out, position.address.as_bytes())?;
            out.extend_from_slice(&position.vote_account);
        }
        Ok(hash48(POLICY_COMMITMENT_DOMAIN, &[&out]))
    }
}

impl SolanaStakeReaderPolicyV1 {
    pub fn validate(&self) -> Result<(), SolanaStakeError> {
        validate_identifier(&self.source_domain)?;
        if self.source_domain != "solana:mainnet" {
            return Err(SolanaStakeError::PolicyMismatch);
        }
        validate_identifier(&self.position_set_id)?;
        for value in [
            &self.stake_program,
            &self.reader_program,
            &self.reader_program_data,
            &self.wallet,
        ] {
            validate_text(value)?;
            decode_pubkey(value)?;
        }
        validate_hex(&self.checkpoint_committee_root, 48)?;
        if decode_pubkey(&self.wallet)? != self.wallet_pubkey
            || self.wallet_pubkey == [0; 32]
            || self.stake_authority == [0; 32]
            || self.withdraw_authority == [0; 32]
            || self.reader_program_data_hash == [0; 32]
            || self.minimum_finalized_depth == 0
            || self.maximum_finalized_slot_lag < u64::from(self.minimum_finalized_depth)
            || self.positions.is_empty()
            || self.positions.len() > MAX_SOLANA_STAKE_ACCOUNTS
        {
            return Err(SolanaStakeError::PolicyMismatch);
        }
        let mut previous_index = None;
        let mut previous_address = None;
        let mut addresses = Vec::with_capacity(self.positions.len());
        for position in &self.positions {
            validate_text(&position.address)?;
            let address = decode_pubkey(&position.address)?;
            if previous_index >= Some(position.index)
                || previous_address >= Some(address)
                || position.vote_account == [0; 32]
                || addresses.contains(&position.address)
            {
                return Err(SolanaStakeError::PolicyMismatch);
            }
            previous_index = Some(position.index);
            previous_address = Some(address);
            addresses.push(position.address.clone());
        }
        Ok(())
    }

    pub fn commitment(&self) -> Result<String, SolanaStakeError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| SolanaStakeError::BoundsExceeded)?;
        Ok(hash48(READER_POLICY_COMMITMENT_DOMAIN, &[&bytes]))
    }
}

pub fn solana_stake_reader_state_commitment_v1(
    proof: &SolanaStakeReaderProofV1,
) -> Result<B256, SolanaStakeError> {
    proof.policy.validate()?;
    validate_reader_bounds(proof)?;
    let mut out = Vec::new();
    append_hex(&mut out, &proof.policy.commitment()?, 48)?;
    append_bytes(&mut out, &proof.policy.reader_program_data_hash)?;
    append_bytes(&mut out, &proof.transaction_signature)?;
    append_bytes(&mut out, &proof.transaction_message)?;
    out.extend_from_slice(&proof.instruction_salt);
    append_bytes(&mut out, &proof.reader_payload)?;
    out.extend_from_slice(&proof.reader_return_data_hash);
    out.extend_from_slice(&proof.reader_slot.to_be_bytes());
    out.extend_from_slice(&proof.reader_epoch.to_be_bytes());
    append_u32(&mut out, proof.positions.len())?;
    for position in &proof.positions {
        append_reader_position(&mut out, position)?;
    }
    Ok(keccak256(domain_message(
        READER_STATE_COMMITMENT_DOMAIN,
        &out,
    )))
}

pub fn solana_stake_reader_owner_statement_v1(
    proof: &SolanaStakeReaderProofV1,
    context: &SolanaStakeVerifyContextV1<'_>,
) -> Result<Vec<u8>, SolanaStakeError> {
    let mut out = context_prefix(context)?;
    append_hex(&mut out, &proof.policy.commitment()?, 48)?;
    out.extend_from_slice(solana_stake_reader_state_commitment_v1(proof)?.as_slice());
    out.extend_from_slice(
        proof
            .checkpoint_certificate
            .checkpoint
            .source_block_hash
            .as_slice(),
    );
    out.extend_from_slice(
        &proof
            .checkpoint_certificate
            .checkpoint
            .source_height
            .to_be_bytes(),
    );
    Ok(domain_message(READER_OWNER_AUTHORIZATION_DOMAIN, &out))
}

pub fn verify_solana_stake_reader_proof_v1(
    proof: &SolanaStakeReaderProofV1,
    context: &SolanaStakeVerifyContextV1<'_>,
) -> Result<SolanaStakeVerificationV1, SolanaStakeError> {
    proof.policy.validate()?;
    validate_reader_bounds(proof)?;
    if proof.policy.source_domain != context.source_domain
        || proof.policy.position_set_id != context.asset_or_position_id
        || solana_stake_owner_commitment(proof.policy.wallet_pubkey)
            != context.reserve_owner_commitment
        || proof.policy.commitment()? != context.quantity_verifier_commitment
    {
        return Err(SolanaStakeError::PolicyMismatch);
    }
    proof
        .checkpoint_certificate
        .verify()
        .map_err(|_| SolanaStakeError::CheckpointMismatch)?;
    let checkpoint = &proof.checkpoint_certificate.checkpoint;
    if checkpoint.pftl_genesis_hash != context.pftl_genesis_hash
        || checkpoint.checkpoint_kind != SOLANA_STAKE_READER_CHECKPOINT_KIND_V1
        || checkpoint.source_domain != proof.policy.source_domain
        || checkpoint.pftl_observation_height != context.observed_at_pftl_height
        || checkpoint.committee_root != proof.policy.checkpoint_committee_root
        || checkpoint.minimum_depth < proof.policy.minimum_finalized_depth
        || checkpoint
            .observed_source_head
            .saturating_sub(checkpoint.source_height)
            > proof.policy.maximum_finalized_slot_lag
        || checkpoint.source_state_commitment != solana_stake_reader_state_commitment_v1(proof)?
    {
        return Err(SolanaStakeError::CheckpointMismatch);
    }
    verify_ed25519(
        proof.policy.withdraw_authority,
        &solana_stake_reader_owner_statement_v1(proof, context)?,
        &proof.ownership_signature,
    )
    .map_err(|_| SolanaStakeError::OwnerAuthorization)?;
    verify_reader_transaction(proof)?;
    let slot = proof.reader_slot;
    let current_epoch = proof.reader_epoch;
    if slot != checkpoint.source_height || proof.positions.len() != proof.policy.positions.len() {
        return Err(SolanaStakeError::PositionMismatch);
    }
    let expected_payload = reader_payload(proof, slot, current_epoch)?;
    let payload_hash: [u8; 32] = Sha256::digest(&proof.reader_payload).into();
    if proof.reader_payload != expected_payload || payload_hash != proof.reader_return_data_hash {
        return Err(SolanaStakeError::ReaderMismatch);
    }
    let evidence_commitment = proof.evidence_commitment()?;
    if evidence_commitment != context.expected_evidence_commitment {
        return Err(SolanaStakeError::EvidenceCommitment);
    }
    let mut total = 0u64;
    let mut locked = 0u64;
    let mut liquid = 0u64;
    let mut positions = Vec::with_capacity(proof.positions.len());
    for (position, snapshot) in proof.policy.positions.iter().zip(&proof.positions) {
        if snapshot.index != position.index || snapshot.address != position.address {
            return Err(SolanaStakeError::PositionMismatch);
        }
        let parsed = parse_reader_position(&proof.policy, position, snapshot, current_epoch)?;
        total = total
            .checked_add(parsed.total_lamports)
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        locked = locked
            .checked_add(parsed.locked_lamports)
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        liquid = liquid
            .checked_add(parsed.liquid_lamports)
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        positions.push(parsed);
    }
    if locked
        .checked_add(liquid)
        .ok_or(SolanaStakeError::ArithmeticOverflow)?
        != total
    {
        return Err(SolanaStakeError::StakeState);
    }
    Ok(SolanaStakeVerificationV1 {
        finalized_slot: slot,
        current_epoch,
        total_lamports: total,
        locked_lamports: locked,
        liquid_lamports: liquid,
        positions,
        evidence_commitment,
    })
}

impl SolanaStakeReaderProofV1 {
    pub fn evidence_commitment(&self) -> Result<String, SolanaStakeError> {
        let state = solana_stake_reader_state_commitment_v1(self)?;
        let certificate = serde_json::to_vec(&self.checkpoint_certificate)
            .map_err(|_| SolanaStakeError::BoundsExceeded)?;
        Ok(hash48(
            READER_EVIDENCE_COMMITMENT_DOMAIN,
            &[state.as_slice(), &certificate, &self.ownership_signature],
        ))
    }
}

pub fn solana_stake_owner_commitment(wallet_pubkey: [u8; 32]) -> String {
    hash48(OWNER_COMMITMENT_DOMAIN, &[&wallet_pubkey])
}

pub fn verify_solana_stake_attested_proof_v1(
    proof: &SolanaStakeAttestedProofV1,
    context: &SolanaStakeVerifyContextV1<'_>,
) -> Result<SolanaStakeVerificationV1, SolanaStakeError> {
    proof.policy.validate()?;
    validate_bounds(proof)?;
    if proof.policy.source_domain != context.source_domain
        || proof.policy.position_set_id != context.asset_or_position_id
        || solana_stake_owner_commitment(proof.policy.wallet_pubkey)
            != context.reserve_owner_commitment
        || proof.policy.commitment()? != context.quantity_verifier_commitment
        || proof.attestations.len() != proof.policy.attestors.len()
    {
        return Err(SolanaStakeError::PolicyMismatch);
    }
    let evidence_commitment = proof.evidence_commitment()?;
    if evidence_commitment != context.expected_evidence_commitment {
        return Err(SolanaStakeError::EvidenceCommitment);
    }
    verify_owner(proof, context)?;
    let canonical = proof
        .attestations
        .first()
        .ok_or(SolanaStakeError::InsufficientAttestations)?;
    for (policy, attestation) in proof.policy.attestors.iter().zip(&proof.attestations) {
        if policy.source_id != attestation.source_id
            || policy.kind != attestation.kind
            || policy.public_key != attestation.signer_public_key
        {
            return Err(SolanaStakeError::AttestorMismatch);
        }
        let statement = attestation_statement(proof, attestation, context)?;
        verify_ed25519(
            attestation.signer_public_key,
            &statement,
            &attestation.signature,
        )
        .map_err(|_| SolanaStakeError::AttestationSignature)?;
        if attestation.finalized_slot != canonical.finalized_slot
            || attestation.clock_sysvar_data != canonical.clock_sysvar_data
            || attestation.stake_accounts != canonical.stake_accounts
        {
            return Err(SolanaStakeError::AttestationDisagreement);
        }
    }
    if canonical.finalized_slot == 0
        || canonical.stake_accounts.len() != proof.policy.positions.len()
    {
        return Err(SolanaStakeError::PositionMismatch);
    }
    let current_epoch = parse_clock_epoch(&canonical.clock_sysvar_data)?;
    let mut total = 0u64;
    let mut locked = 0u64;
    let mut liquid = 0u64;
    let mut positions = Vec::with_capacity(canonical.stake_accounts.len());
    for (policy, snapshot) in proof.policy.positions.iter().zip(&canonical.stake_accounts) {
        if snapshot.index != policy.index || snapshot.address != policy.address {
            return Err(SolanaStakeError::PositionMismatch);
        }
        let parsed = parse_stake_position(&proof.policy, policy, snapshot, current_epoch)?;
        total = total
            .checked_add(parsed.total_lamports)
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        locked = locked
            .checked_add(parsed.locked_lamports)
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        liquid = liquid
            .checked_add(parsed.liquid_lamports)
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        positions.push(parsed);
    }
    if locked
        .checked_add(liquid)
        .ok_or(SolanaStakeError::ArithmeticOverflow)?
        != total
    {
        return Err(SolanaStakeError::StakeState);
    }
    Ok(SolanaStakeVerificationV1 {
        finalized_slot: canonical.finalized_slot,
        current_epoch,
        total_lamports: total,
        locked_lamports: locked,
        liquid_lamports: liquid,
        positions,
        evidence_commitment,
    })
}

impl SolanaStakeAttestedProofV1 {
    pub fn evidence_commitment(&self) -> Result<String, SolanaStakeError> {
        self.policy.validate()?;
        validate_bounds(self)?;
        let mut out = Vec::new();
        append_hex(&mut out, &self.policy.commitment()?, 48)?;
        append_bytes(&mut out, &self.ownership_signature)?;
        append_u32(&mut out, self.attestations.len())?;
        for attestation in &self.attestations {
            append_attestation(&mut out, attestation)?;
        }
        Ok(hash48(EVIDENCE_COMMITMENT_DOMAIN, &[&out]))
    }
}

fn verify_owner(
    proof: &SolanaStakeAttestedProofV1,
    context: &SolanaStakeVerifyContextV1<'_>,
) -> Result<(), SolanaStakeError> {
    let statement = owner_statement(proof, context)?;
    verify_ed25519(
        proof.policy.withdraw_authority,
        &statement,
        &proof.ownership_signature,
    )
    .map_err(|_| SolanaStakeError::OwnerAuthorization)
}

fn owner_statement(
    proof: &SolanaStakeAttestedProofV1,
    context: &SolanaStakeVerifyContextV1<'_>,
) -> Result<Vec<u8>, SolanaStakeError> {
    let mut out = context_prefix(context)?;
    append_hex(&mut out, &proof.policy.commitment()?, 48)?;
    append_u32(&mut out, proof.attestations.len())?;
    for attestation in &proof.attestations {
        append_attestation_payload(&mut out, attestation)?;
    }
    Ok(domain_message(OWNER_AUTHORIZATION_DOMAIN, &out))
}

fn attestation_statement(
    proof: &SolanaStakeAttestedProofV1,
    attestation: &SolanaStakeAttestationV1,
    context: &SolanaStakeVerifyContextV1<'_>,
) -> Result<Vec<u8>, SolanaStakeError> {
    let mut out = context_prefix(context)?;
    append_hex(&mut out, &proof.policy.commitment()?, 48)?;
    append_attestation_payload(&mut out, attestation)?;
    Ok(domain_message(ATTESTATION_DOMAIN, &out))
}

fn context_prefix(context: &SolanaStakeVerifyContextV1<'_>) -> Result<Vec<u8>, SolanaStakeError> {
    let mut out = Vec::new();
    for (value, bytes) in [
        (context.pftl_genesis_hash, 48usize),
        (context.nav_asset_id, 48),
        (context.proof_profile_id, 48),
        (context.valuation_policy_hash, 32),
        (context.source_manifest_hash, 48),
    ] {
        append_hex(&mut out, value, bytes)?;
    }
    validate_identifier(context.source_id)?;
    append_bytes(&mut out, context.source_id.as_bytes())?;
    out.extend_from_slice(&context.observed_at_pftl_height.to_be_bytes());
    Ok(out)
}

fn parse_stake_position(
    policy: &SolanaStakePolicyV1,
    position: &SolanaStakePositionPolicyV1,
    snapshot: &SolanaStakeAccountSnapshotV1,
    current_epoch: u64,
) -> Result<SolanaParsedStakePositionV1, SolanaStakeError> {
    if !snapshot.exists {
        if snapshot.lamports != 0 || !snapshot.data.is_empty() {
            return Err(SolanaStakeError::StakeState);
        }
        return Ok(SolanaParsedStakePositionV1 {
            index: snapshot.index,
            address: snapshot.address.clone(),
            total_lamports: 0,
            delegated_lamports: 0,
            activation_epoch: 0,
            deactivation_epoch: SOLANA_DEACTIVATION_EPOCH_DISABLED,
            locked_lamports: 0,
            liquid_lamports: 0,
        });
    }
    if snapshot.owner_program != policy.stake_program
        || read_u32_le(&snapshot.data, 0)? != STAKE_STATE_V2_DELEGATED
        || read_pubkey(&snapshot.data, 12)? != policy.stake_authority
        || read_pubkey(&snapshot.data, 44)? != policy.withdraw_authority
        || read_pubkey(&snapshot.data, 124)? != position.vote_account
    {
        return Err(SolanaStakeError::StakeState);
    }
    let delegated = read_u64_le(&snapshot.data, 156)?;
    let activation = read_u64_le(&snapshot.data, 164)?;
    let deactivation = read_u64_le(&snapshot.data, 172)?;
    if delegated > snapshot.lamports {
        return Err(SolanaStakeError::StakeState);
    }
    let is_liquid =
        deactivation != SOLANA_DEACTIVATION_EPOCH_DISABLED && deactivation < current_epoch;
    Ok(SolanaParsedStakePositionV1 {
        index: snapshot.index,
        address: snapshot.address.clone(),
        total_lamports: snapshot.lamports,
        delegated_lamports: delegated,
        activation_epoch: activation,
        deactivation_epoch: deactivation,
        locked_lamports: if is_liquid { 0 } else { snapshot.lamports },
        liquid_lamports: if is_liquid { snapshot.lamports } else { 0 },
    })
}

pub fn derive_stake_address(
    wallet: [u8; 32],
    seed_prefix: &str,
    index: u32,
    stake_program: &str,
) -> Result<String, SolanaStakeError> {
    validate_text(seed_prefix)?;
    let program = decode_pubkey(stake_program)?;
    let seed = format!("{seed_prefix}{index}");
    let mut hasher = Sha256::new();
    hasher.update(wallet);
    hasher.update(seed.as_bytes());
    hasher.update(program);
    Ok(bs58::encode(hasher.finalize()).into_string())
}

fn parse_reader_position(
    policy: &SolanaStakeReaderPolicyV1,
    position: &SolanaStakePositionPolicyV1,
    snapshot: &SolanaStakeReaderPositionV1,
    current_epoch: u64,
) -> Result<SolanaParsedStakePositionV1, SolanaStakeError> {
    if snapshot.owner_program != policy.stake_program
        || snapshot.data_hash == [0; 32]
        || snapshot.stake_authority != policy.stake_authority
        || snapshot.withdraw_authority != policy.withdraw_authority
        || snapshot.vote_account != position.vote_account
    {
        return Err(SolanaStakeError::StakeState);
    }
    if snapshot.delegated_lamports > snapshot.lamports {
        return Err(SolanaStakeError::StakeState);
    }
    let is_liquid = snapshot.deactivation_epoch != SOLANA_DEACTIVATION_EPOCH_DISABLED
        && snapshot.deactivation_epoch < current_epoch;
    Ok(SolanaParsedStakePositionV1 {
        index: snapshot.index,
        address: snapshot.address.clone(),
        total_lamports: snapshot.lamports,
        delegated_lamports: snapshot.delegated_lamports,
        activation_epoch: snapshot.activation_epoch,
        deactivation_epoch: snapshot.deactivation_epoch,
        locked_lamports: if is_liquid { 0 } else { snapshot.lamports },
        liquid_lamports: if is_liquid { snapshot.lamports } else { 0 },
    })
}

fn reader_payload(
    proof: &SolanaStakeReaderProofV1,
    slot: u64,
    epoch: u64,
) -> Result<Vec<u8>, SolanaStakeError> {
    let mut out = Vec::new();
    out.extend_from_slice(READER_SNAPSHOT_MAGIC);
    out.extend_from_slice(&READER_SNAPSHOT_VERSION.to_le_bytes());
    out.extend_from_slice(&slot.to_le_bytes());
    out.extend_from_slice(&epoch.to_le_bytes());
    out.extend_from_slice(&proof.instruction_salt);
    out.extend_from_slice(
        &u16::try_from(proof.positions.len())
            .map_err(|_| SolanaStakeError::BoundsExceeded)?
            .to_le_bytes(),
    );
    for (position, snapshot) in proof.policy.positions.iter().zip(&proof.positions) {
        if snapshot.index != position.index
            || snapshot.address != position.address
            || snapshot.owner_program != proof.policy.stake_program
        {
            return Err(SolanaStakeError::PositionMismatch);
        }
        out.extend_from_slice(&decode_pubkey(&snapshot.address)?);
        out.extend_from_slice(&snapshot.lamports.to_le_bytes());
        out.extend_from_slice(&decode_pubkey(&snapshot.owner_program)?);
        out.extend_from_slice(&snapshot.data_hash);
        out.extend_from_slice(&snapshot.stake_authority);
        out.extend_from_slice(&snapshot.withdraw_authority);
        out.extend_from_slice(&snapshot.vote_account);
        out.extend_from_slice(&snapshot.delegated_lamports.to_le_bytes());
        out.extend_from_slice(&snapshot.activation_epoch.to_le_bytes());
        out.extend_from_slice(&snapshot.deactivation_epoch.to_le_bytes());
    }
    Ok(out)
}

fn verify_reader_transaction(proof: &SolanaStakeReaderProofV1) -> Result<(), SolanaStakeError> {
    let message = parse_legacy_message(&proof.transaction_message)?;
    let expected_account_count = proof
        .policy
        .positions
        .len()
        .checked_add(3)
        .ok_or(SolanaStakeError::ArithmeticOverflow)?;
    if message.required_signatures != 1
        || message.readonly_signed_accounts != 0
        || usize::from(message.readonly_unsigned_accounts) != expected_account_count - 1
        || message.account_keys.len() != expected_account_count
        || message.recent_blockhash == [0; 32]
    {
        return Err(SolanaStakeError::Transaction);
    }
    for index in 0..message.account_keys.len() {
        if message.account_keys[..index].contains(&message.account_keys[index]) {
            return Err(SolanaStakeError::Transaction);
        }
    }
    verify_ed25519(
        message.account_keys[0],
        &proof.transaction_message,
        &proof.transaction_signature,
    )
    .map_err(|_| SolanaStakeError::Transaction)?;
    if message.instructions.len() != 1 {
        return Err(SolanaStakeError::Transaction);
    }
    let instruction = &message.instructions[0];
    let expected_program_index =
        u8::try_from(expected_account_count - 1).map_err(|_| SolanaStakeError::BoundsExceeded)?;
    if instruction.program_id_index != expected_program_index {
        return Err(SolanaStakeError::Transaction);
    }
    let program = *message
        .account_keys
        .get(usize::from(instruction.program_id_index))
        .ok_or(SolanaStakeError::Transaction)?;
    if program != decode_pubkey(&proof.policy.reader_program)? {
        return Err(SolanaStakeError::ReaderMismatch);
    }
    let expected_accounts = std::iter::once(decode_pubkey(SOLANA_CLOCK_SYSVAR_ID)?)
        .chain(
            proof
                .policy
                .positions
                .iter()
                .map(|position| decode_pubkey(&position.address))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .collect::<Vec<_>>();
    let actual_accounts = instruction
        .account_indices
        .iter()
        .map(|index| {
            message
                .account_keys
                .get(usize::from(*index))
                .copied()
                .ok_or(SolanaStakeError::Transaction)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_indices = (1..expected_account_count - 1)
        .map(|index| u8::try_from(index).map_err(|_| SolanaStakeError::BoundsExceeded))
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected_data = Vec::from(READER_INSTRUCTION_MAGIC);
    expected_data.extend_from_slice(&proof.instruction_salt);
    expected_data.extend_from_slice(
        &u16::try_from(proof.policy.positions.len())
            .map_err(|_| SolanaStakeError::BoundsExceeded)?
            .to_le_bytes(),
    );
    if actual_accounts != expected_accounts
        || instruction.account_indices != expected_indices
        || instruction.data != expected_data
    {
        return Err(SolanaStakeError::ReaderMismatch);
    }
    Ok(())
}

struct LegacyMessage {
    required_signatures: u8,
    readonly_signed_accounts: u8,
    readonly_unsigned_accounts: u8,
    account_keys: Vec<[u8; 32]>,
    recent_blockhash: [u8; 32],
    instructions: Vec<LegacyInstruction>,
}

struct LegacyInstruction {
    program_id_index: u8,
    account_indices: Vec<u8>,
    data: Vec<u8>,
}

fn parse_legacy_message(bytes: &[u8]) -> Result<LegacyMessage, SolanaStakeError> {
    if bytes.len() < 3 || bytes[0] & 0x80 != 0 {
        return Err(SolanaStakeError::Transaction);
    }
    let mut reader = SolanaReader::new(bytes);
    let required_signatures = reader.byte()?;
    let readonly_signed_accounts = reader.byte()?;
    let readonly_unsigned_accounts = reader.byte()?;
    let account_count = reader.short_vec()?;
    if required_signatures == 0
        || account_count == 0
        || account_count > MAX_SOLANA_STAKE_ACCOUNTS + 4
    {
        return Err(SolanaStakeError::Transaction);
    }
    let mut account_keys = Vec::with_capacity(account_count);
    for _ in 0..account_count {
        account_keys.push(reader.array_32()?);
    }
    let recent_blockhash = reader.array_32()?;
    let instruction_count = reader.short_vec()?;
    if instruction_count == 0 || instruction_count > 8 {
        return Err(SolanaStakeError::Transaction);
    }
    let mut instructions = Vec::with_capacity(instruction_count);
    for _ in 0..instruction_count {
        let program_id_index = reader.byte()?;
        let account_len = reader.short_vec()?;
        if account_len > MAX_SOLANA_STAKE_ACCOUNTS + 1 {
            return Err(SolanaStakeError::Transaction);
        }
        let account_indices = reader.bytes(account_len)?.to_vec();
        let data_len = reader.short_vec()?;
        if data_len > 1024 {
            return Err(SolanaStakeError::Transaction);
        }
        let data = reader.bytes(data_len)?.to_vec();
        instructions.push(LegacyInstruction {
            program_id_index,
            account_indices,
            data,
        });
    }
    reader.finish()?;
    Ok(LegacyMessage {
        required_signatures,
        readonly_signed_accounts,
        readonly_unsigned_accounts,
        account_keys,
        recent_blockhash,
        instructions,
    })
}

struct SolanaReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SolanaReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn byte(&mut self) -> Result<u8, SolanaStakeError> {
        let value = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(SolanaStakeError::Transaction)?;
        self.offset += 1;
        Ok(value)
    }

    fn short_vec(&mut self) -> Result<usize, SolanaStakeError> {
        let start = self.offset;
        let mut value = 0usize;
        let mut shift = 0u32;
        loop {
            let byte = self.byte()?;
            if shift >= usize::BITS || (shift + 7 >= usize::BITS && byte > 1) {
                return Err(SolanaStakeError::Transaction);
            }
            value |= usize::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                let mut canonical = Vec::new();
                write_short_vec(value, &mut canonical);
                if self.bytes[start..self.offset] != canonical {
                    return Err(SolanaStakeError::Transaction);
                }
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], SolanaStakeError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(SolanaStakeError::Transaction)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(SolanaStakeError::Transaction)?;
        self.offset = end;
        Ok(value)
    }

    fn array_32(&mut self) -> Result<[u8; 32], SolanaStakeError> {
        self.bytes(32)?
            .try_into()
            .map_err(|_| SolanaStakeError::Transaction)
    }

    fn finish(&self) -> Result<(), SolanaStakeError> {
        if self.offset != self.bytes.len() {
            return Err(SolanaStakeError::Transaction);
        }
        Ok(())
    }
}

fn write_short_vec(mut value: usize, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn validate_reader_bounds(proof: &SolanaStakeReaderProofV1) -> Result<(), SolanaStakeError> {
    if proof.ownership_signature.len() != 64
        || proof.transaction_signature.len() != 64
        || proof.transaction_message.is_empty()
        || proof.transaction_message.len() > 4096
        || proof.instruction_salt == [0; 32]
        || proof.reader_payload.is_empty()
        || proof.reader_payload.len() > 16 * 1024
        || proof.reader_slot == 0
        || proof.positions.len() != proof.policy.positions.len()
    {
        return Err(SolanaStakeError::BoundsExceeded);
    }
    let mut total = 0usize;
    for position in &proof.positions {
        validate_text(&position.address)?;
        validate_text(&position.owner_program)?;
        if position.data_hash == [0; 32] {
            return Err(SolanaStakeError::BoundsExceeded);
        }
        total = total
            .checked_add(196)
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
    }
    if total > MAX_SOLANA_TOTAL_DATA_BYTES {
        return Err(SolanaStakeError::BoundsExceeded);
    }
    Ok(())
}

fn append_reader_position(
    out: &mut Vec<u8>,
    position: &SolanaStakeReaderPositionV1,
) -> Result<(), SolanaStakeError> {
    out.extend_from_slice(&position.index.to_be_bytes());
    append_bytes(out, position.address.as_bytes())?;
    out.extend_from_slice(&position.lamports.to_be_bytes());
    append_bytes(out, position.owner_program.as_bytes())?;
    out.extend_from_slice(&position.data_hash);
    out.extend_from_slice(&position.stake_authority);
    out.extend_from_slice(&position.withdraw_authority);
    out.extend_from_slice(&position.vote_account);
    out.extend_from_slice(&position.delegated_lamports.to_be_bytes());
    out.extend_from_slice(&position.activation_epoch.to_be_bytes());
    out.extend_from_slice(&position.deactivation_epoch.to_be_bytes());
    Ok(())
}

fn validate_bounds(proof: &SolanaStakeAttestedProofV1) -> Result<(), SolanaStakeError> {
    if proof.ownership_signature.len() != 64
        || proof.attestations.is_empty()
        || proof.attestations.len() > MAX_SOLANA_ATTESTORS
    {
        return Err(SolanaStakeError::BoundsExceeded);
    }
    let mut total = 0usize;
    for attestation in &proof.attestations {
        validate_identifier(&attestation.source_id)?;
        if attestation.signature.len() != 64
            || attestation.clock_sysvar_data.len() < CLOCK_MIN_LEN
            || attestation.clock_sysvar_data.len() > MAX_SOLANA_ACCOUNT_DATA_BYTES
            || attestation.stake_accounts.len() > MAX_SOLANA_STAKE_ACCOUNTS
        {
            return Err(SolanaStakeError::BoundsExceeded);
        }
        total = total
            .checked_add(attestation.clock_sysvar_data.len())
            .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        for account in &attestation.stake_accounts {
            validate_text(&account.address)?;
            validate_text(&account.owner_program)?;
            if account.data.len() > MAX_SOLANA_ACCOUNT_DATA_BYTES {
                return Err(SolanaStakeError::BoundsExceeded);
            }
            total = total
                .checked_add(account.data.len())
                .ok_or(SolanaStakeError::ArithmeticOverflow)?;
        }
    }
    if total > MAX_SOLANA_TOTAL_DATA_BYTES {
        return Err(SolanaStakeError::BoundsExceeded);
    }
    Ok(())
}

fn append_attestation(
    out: &mut Vec<u8>,
    value: &SolanaStakeAttestationV1,
) -> Result<(), SolanaStakeError> {
    append_attestation_payload(out, value)?;
    append_bytes(out, &value.signature)
}

fn append_attestation_payload(
    out: &mut Vec<u8>,
    value: &SolanaStakeAttestationV1,
) -> Result<(), SolanaStakeError> {
    append_bytes(out, value.source_id.as_bytes())?;
    out.push(value.kind.tag());
    out.extend_from_slice(&value.signer_public_key);
    out.extend_from_slice(&value.finalized_slot.to_be_bytes());
    append_bytes(out, &value.clock_sysvar_data)?;
    append_u32(out, value.stake_accounts.len())?;
    for account in &value.stake_accounts {
        out.extend_from_slice(&account.index.to_be_bytes());
        append_bytes(out, account.address.as_bytes())?;
        out.push(u8::from(account.exists));
        out.extend_from_slice(&account.lamports.to_be_bytes());
        append_bytes(out, account.owner_program.as_bytes())?;
        append_bytes(out, &account.data)?;
    }
    Ok(())
}

fn verify_ed25519(public_key: [u8; 32], message: &[u8], signature: &[u8]) -> Result<(), ()> {
    let key = VerifyingKey::from_bytes(&public_key).map_err(|_| ())?;
    let signature: [u8; 64] = signature.try_into().map_err(|_| ())?;
    key.verify(message, &Signature::from_bytes(&signature))
        .map_err(|_| ())
}

fn parse_clock_epoch(data: &[u8]) -> Result<u64, SolanaStakeError> {
    if data.len() < CLOCK_MIN_LEN {
        return Err(SolanaStakeError::Clock);
    }
    read_u64_le(data, CLOCK_EPOCH_OFFSET).map_err(|_| SolanaStakeError::Clock)
}

fn read_pubkey(data: &[u8], offset: usize) -> Result<[u8; 32], SolanaStakeError> {
    data.get(offset..offset + 32)
        .ok_or(SolanaStakeError::StakeState)?
        .try_into()
        .map_err(|_| SolanaStakeError::StakeState)
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, SolanaStakeError> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or(SolanaStakeError::StakeState)?
            .try_into()
            .map_err(|_| SolanaStakeError::StakeState)?,
    ))
}

fn read_u64_le(data: &[u8], offset: usize) -> Result<u64, SolanaStakeError> {
    Ok(u64::from_le_bytes(
        data.get(offset..offset + 8)
            .ok_or(SolanaStakeError::StakeState)?
            .try_into()
            .map_err(|_| SolanaStakeError::StakeState)?,
    ))
}

fn decode_pubkey(value: &str) -> Result<[u8; 32], SolanaStakeError> {
    bs58::decode(value)
        .into_vec()
        .map_err(|_| SolanaStakeError::PolicyMismatch)?
        .try_into()
        .map_err(|_| SolanaStakeError::PolicyMismatch)
}

fn validate_identifier(value: &str) -> Result<(), SolanaStakeError> {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(SolanaStakeError::PolicyMismatch);
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), SolanaStakeError> {
    if value.is_empty() || value.len() > 256 || !value.is_ascii() {
        return Err(SolanaStakeError::PolicyMismatch);
    }
    Ok(())
}

fn validate_hex(value: &str, bytes: usize) -> Result<(), SolanaStakeError> {
    if value.len() != bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SolanaStakeError::PolicyMismatch);
    }
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), SolanaStakeError> {
    validate_hex(value, bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| SolanaStakeError::PolicyMismatch)?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), SolanaStakeError> {
    let len = u32::try_from(value.len()).map_err(|_| SolanaStakeError::BoundsExceeded)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), SolanaStakeError> {
    let value = u32::try_from(value).map_err(|_| SolanaStakeError::BoundsExceeded)?;
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
    Sha3Digest::update(&mut hasher, (domain.len() as u32).to_be_bytes());
    Sha3Digest::update(&mut hasher, domain);
    for part in parts {
        Sha3Digest::update(&mut hasher, (part.len() as u64).to_be_bytes());
        Sha3Digest::update(&mut hasher, part);
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bft_checkpoint::{
        BftCheckpointCommitteeV1, BftCheckpointValidatorV1, BftSourceCheckpointCertificateV1,
        BftSourceCheckpointV1, BftSourceCheckpointVoteV1,
        BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    const STAKE_PROGRAM: &str = "Stake11111111111111111111111111111111111111";

    fn context<'a>(
        proof: &SolanaStakeAttestedProofV1,
        commitment: &'a str,
    ) -> SolanaStakeVerifyContextV1<'a> {
        SolanaStakeVerifyContextV1 {
            pftl_genesis_hash: "11".repeat(48).leak(),
            nav_asset_id: "22".repeat(48).leak(),
            proof_profile_id: "33".repeat(48).leak(),
            valuation_policy_hash: "44".repeat(32).leak(),
            source_manifest_hash: "55".repeat(48).leak(),
            source_id: "solana-stake-primary",
            source_domain: proof.policy.source_domain.clone().leak(),
            asset_or_position_id: proof.policy.position_set_id.clone().leak(),
            reserve_owner_commitment: solana_stake_owner_commitment(proof.policy.wallet_pubkey)
                .leak(),
            quantity_verifier_commitment: proof.policy.commitment().unwrap().leak(),
            observed_at_pftl_height: 500,
            expected_evidence_commitment: commitment,
        }
    }

    fn stake_data(authority: [u8; 32], vote: [u8; 32], deactivation: u64) -> Vec<u8> {
        let mut data = vec![0; 200];
        data[0..4].copy_from_slice(&STAKE_STATE_V2_DELEGATED.to_le_bytes());
        data[12..44].copy_from_slice(&authority);
        data[44..76].copy_from_slice(&authority);
        data[124..156].copy_from_slice(&vote);
        data[156..164].copy_from_slice(&9_900_000_000u64.to_le_bytes());
        data[164..172].copy_from_slice(&100u64.to_le_bytes());
        data[172..180].copy_from_slice(&deactivation.to_le_bytes());
        data
    }

    fn fixture() -> (SolanaStakeAttestedProofV1, SigningKey, Vec<SigningKey>) {
        let wallet_key = SigningKey::from_bytes(&[8; 32]);
        let wallet_pubkey = *wallet_key.verifying_key().as_bytes();
        let wallet = bs58::encode(wallet_pubkey).into_string();
        let vote = [9; 32];
        let position = SolanaStakePositionPolicyV1 {
            index: 0,
            address: derive_stake_address(wallet_pubkey, "postfiat:", 0, STAKE_PROGRAM).unwrap(),
            vote_account: vote,
        };
        let keys = vec![
            SigningKey::from_bytes(&[1; 32]),
            SigningKey::from_bytes(&[2; 32]),
            SigningKey::from_bytes(&[3; 32]),
        ];
        let attestors = vec![
            ("own", SolanaAttestorKindV1::OwnNode),
            ("public-a", SolanaAttestorKindV1::PublicRpc),
            ("public-b", SolanaAttestorKindV1::PublicRpc),
        ]
        .into_iter()
        .zip(&keys)
        .map(|((source_id, kind), key)| SolanaAttestorPolicyV1 {
            source_id: source_id.to_string(),
            kind,
            public_key: *key.verifying_key().as_bytes(),
        })
        .collect::<Vec<_>>();
        let policy = SolanaStakePolicyV1 {
            source_domain: "solana:mainnet-beta".to_string(),
            position_set_id: "solana-stake-set:a666-v1".to_string(),
            stake_program: STAKE_PROGRAM.to_string(),
            stake_seed_prefix: "postfiat:".to_string(),
            wallet,
            wallet_pubkey,
            stake_authority: wallet_pubkey,
            withdraw_authority: wallet_pubkey,
            allow_self_attested: false,
            minimum_own_nodes: 1,
            minimum_public_rpcs: 2,
            attestors,
            positions: vec![position.clone()],
        };
        let mut clock = vec![0; CLOCK_MIN_LEN];
        clock[CLOCK_EPOCH_OFFSET..CLOCK_EPOCH_OFFSET + 8].copy_from_slice(&500u64.to_le_bytes());
        let snapshot = SolanaStakeAccountSnapshotV1 {
            index: 0,
            address: position.address,
            exists: true,
            lamports: 10_000_000_000,
            owner_program: STAKE_PROGRAM.to_string(),
            data: stake_data(wallet_pubkey, vote, SOLANA_DEACTIVATION_EPOCH_DISABLED),
        };
        let attestations = policy
            .attestors
            .iter()
            .map(|attestor| SolanaStakeAttestationV1 {
                source_id: attestor.source_id.clone(),
                kind: attestor.kind,
                signer_public_key: attestor.public_key,
                signature: vec![0; 64],
                finalized_slot: 77,
                clock_sysvar_data: clock.clone(),
                stake_accounts: vec![snapshot.clone()],
            })
            .collect();
        (
            SolanaStakeAttestedProofV1 {
                policy,
                ownership_signature: vec![0; 64],
                attestations,
            },
            wallet_key,
            keys,
        )
    }

    fn authorize(proof: &mut SolanaStakeAttestedProofV1, owner: &SigningKey, keys: &[SigningKey]) {
        let context = context(proof, "00");
        let statements = proof
            .attestations
            .iter()
            .map(|attestation| attestation_statement(proof, attestation, &context).unwrap())
            .collect::<Vec<_>>();
        for ((attestation, key), statement) in
            proof.attestations.iter_mut().zip(keys).zip(statements)
        {
            attestation.signature = key.sign(&statement).to_bytes().to_vec();
        }
        let statement = owner_statement(proof, &context).unwrap();
        proof.ownership_signature = owner.sign(&statement).to_bytes().to_vec();
    }

    fn checkpoint_committee() -> (BftCheckpointCommitteeV1, Vec<MlDsa65KeyPair>) {
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        (
            BftCheckpointCommitteeV1 {
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
            },
            keys,
        )
    }

    fn reader_context<'a>(
        proof: &SolanaStakeReaderProofV1,
        commitment: &'a str,
    ) -> SolanaStakeVerifyContextV1<'a> {
        SolanaStakeVerifyContextV1 {
            pftl_genesis_hash: "11".repeat(48).leak(),
            nav_asset_id: "22".repeat(48).leak(),
            proof_profile_id: "33".repeat(48).leak(),
            valuation_policy_hash: "44".repeat(32).leak(),
            source_manifest_hash: "55".repeat(48).leak(),
            source_id: "solana-stake-primary",
            source_domain: proof.policy.source_domain.clone().leak(),
            asset_or_position_id: proof.policy.position_set_id.clone().leak(),
            reserve_owner_commitment: solana_stake_owner_commitment(proof.policy.wallet_pubkey)
                .leak(),
            quantity_verifier_commitment: proof.policy.commitment().unwrap().leak(),
            observed_at_pftl_height: 500,
            expected_evidence_commitment: commitment,
        }
    }

    fn reader_fixture() -> SolanaStakeReaderProofV1 {
        let owner = SigningKey::from_bytes(&[8; 32]);
        let owner_pubkey = *owner.verifying_key().as_bytes();
        let position_address = [7u8; 32];
        let vote = [9u8; 32];
        let reader_program = [4u8; 32];
        let reader_program_data = [5u8; 32];
        let (committee, checkpoint_keys) = checkpoint_committee();
        let policy = SolanaStakeReaderPolicyV1 {
            source_domain: "solana:mainnet".to_string(),
            position_set_id: "solana-stake-set:a666-v2".to_string(),
            stake_program: STAKE_PROGRAM.to_string(),
            reader_program: bs58::encode(reader_program).into_string(),
            reader_program_data: bs58::encode(reader_program_data).into_string(),
            reader_program_data_hash: [6; 32],
            wallet: bs58::encode(owner_pubkey).into_string(),
            wallet_pubkey: owner_pubkey,
            stake_authority: owner_pubkey,
            withdraw_authority: owner_pubkey,
            positions: vec![SolanaStakePositionPolicyV1 {
                index: 0,
                address: bs58::encode(position_address).into_string(),
                vote_account: vote,
            }],
            checkpoint_committee_root: committee.root().unwrap(),
            minimum_finalized_depth: 32,
            maximum_finalized_slot_lag: 512,
        };
        let stake_account_data = stake_data(owner_pubkey, vote, SOLANA_DEACTIVATION_EPOCH_DISABLED);
        let position = SolanaStakeReaderPositionV1 {
            index: 0,
            address: bs58::encode(position_address).into_string(),
            lamports: 10_000_000_000,
            owner_program: STAKE_PROGRAM.to_string(),
            data_hash: Sha256::digest(&stake_account_data).into(),
            stake_authority: owner_pubkey,
            withdraw_authority: owner_pubkey,
            vote_account: vote,
            delegated_lamports: 9_900_000_000,
            activation_epoch: 100,
            deactivation_epoch: SOLANA_DEACTIVATION_EPOCH_DISABLED,
        };
        let salt = [0x42; 32];
        let clock_key = decode_pubkey(SOLANA_CLOCK_SYSVAR_ID).unwrap();
        let mut instruction_data = Vec::from(READER_INSTRUCTION_MAGIC);
        instruction_data.extend_from_slice(&salt);
        instruction_data.extend_from_slice(&1u16.to_le_bytes());
        let mut message = vec![1, 0, 3, 4];
        message.extend_from_slice(&owner_pubkey);
        message.extend_from_slice(&clock_key);
        message.extend_from_slice(&position_address);
        message.extend_from_slice(&reader_program);
        message.extend_from_slice(&[0x66; 32]);
        message.push(1);
        message.push(3);
        message.push(2);
        message.extend_from_slice(&[1, 2]);
        message.push(u8::try_from(instruction_data.len()).unwrap());
        message.extend_from_slice(&instruction_data);
        let transaction_signature = owner.sign(&message).to_bytes().to_vec();
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: SOLANA_STAKE_READER_CHECKPOINT_KIND_V1.to_string(),
            source_domain: policy.source_domain.clone(),
            source_height: 77,
            source_timestamp_ms: 1_785_000_000_000,
            source_block_hash: B256::repeat_byte(0x77),
            source_state_commitment: B256::repeat_byte(1),
            observed_source_head: 109,
            minimum_depth: 32,
            pftl_observation_height: 500,
            committee_epoch: committee.epoch,
            committee_root: committee.root().unwrap(),
        };
        let certificate = BftSourceCheckpointCertificateV1 {
            committee,
            checkpoint,
            votes: Vec::new(),
        };
        let mut proof = SolanaStakeReaderProofV1 {
            policy,
            checkpoint_certificate: certificate,
            ownership_signature: vec![0; 64],
            transaction_signature,
            transaction_message: message,
            instruction_salt: salt,
            reader_payload: Vec::new(),
            reader_return_data_hash: [0; 32],
            reader_slot: 77,
            reader_epoch: 500,
            positions: vec![position],
        };
        proof.reader_payload = reader_payload(&proof, 77, 500).unwrap();
        proof.reader_return_data_hash = Sha256::digest(&proof.reader_payload).into();
        proof
            .checkpoint_certificate
            .checkpoint
            .source_state_commitment = solana_stake_reader_state_commitment_v1(&proof).unwrap();
        proof.checkpoint_certificate.votes = (0..3)
            .map(|index| {
                let validator_id = format!("validator-{index}");
                let statement = proof
                    .checkpoint_certificate
                    .checkpoint
                    .vote_signing_statement(&validator_id)
                    .unwrap();
                BftSourceCheckpointVoteV1 {
                    validator_id,
                    signature: ml_dsa_65_sign_with_context_seed(
                        &checkpoint_keys[index].private_key,
                        &statement,
                        BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                        &[0x80 + index as u8; 32],
                    )
                    .unwrap(),
                }
            })
            .collect();
        let statement =
            solana_stake_reader_owner_statement_v1(&proof, &reader_context(&proof, "")).unwrap();
        proof.ownership_signature = owner.sign(&statement).to_bytes().to_vec();
        proof
    }

    fn resign_reader_checkpoint_and_owner(proof: &mut SolanaStakeReaderProofV1) {
        let (_, checkpoint_keys) = checkpoint_committee();
        proof.checkpoint_certificate.votes = (0..3)
            .map(|index| {
                let validator_id = format!("validator-{index}");
                let statement = proof
                    .checkpoint_certificate
                    .checkpoint
                    .vote_signing_statement(&validator_id)
                    .unwrap();
                BftSourceCheckpointVoteV1 {
                    validator_id,
                    signature: ml_dsa_65_sign_with_context_seed(
                        &checkpoint_keys[index].private_key,
                        &statement,
                        BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                        &[0xa0 + index as u8; 32],
                    )
                    .unwrap(),
                }
            })
            .collect();
        let owner = SigningKey::from_bytes(&[8; 32]);
        let statement =
            solana_stake_reader_owner_statement_v1(proof, &reader_context(proof, "")).unwrap();
        proof.ownership_signature = owner.sign(&statement).to_bytes().to_vec();
    }

    #[test]
    fn verifies_independent_attestors_owner_and_stake_state() {
        let (mut proof, owner, keys) = fixture();
        authorize(&mut proof, &owner, &keys);
        let commitment = proof.evidence_commitment().unwrap();
        let verified =
            verify_solana_stake_attested_proof_v1(&proof, &context(&proof, &commitment)).unwrap();
        assert_eq!(verified.total_lamports, 10_000_000_000);
        assert_eq!(verified.locked_lamports, 10_000_000_000);
        assert_eq!(verified.liquid_lamports, 0);
    }

    #[test]
    fn verifies_public_reader_transaction_checkpoint_owner_and_reader_state() {
        let proof = reader_fixture();
        let commitment = proof.evidence_commitment().unwrap();
        let verified =
            verify_solana_stake_reader_proof_v1(&proof, &reader_context(&proof, &commitment))
                .unwrap();
        assert_eq!(verified.finalized_slot, 77);
        assert_eq!(verified.total_lamports, 10_000_000_000);
        assert_eq!(verified.locked_lamports, 10_000_000_000);
    }

    #[test]
    fn reader_proof_rejects_transaction_payload_state_and_checkpoint_substitution() {
        let proof = reader_fixture();

        let mut tampered = proof.clone();
        tampered.transaction_message[3] ^= 1;
        let commitment = tampered.evidence_commitment().unwrap();
        assert_eq!(
            verify_solana_stake_reader_proof_v1(&tampered, &reader_context(&tampered, &commitment)),
            Err(SolanaStakeError::CheckpointMismatch)
        );

        let mut tampered = proof.clone();
        tampered.reader_payload[0] ^= 1;
        let commitment = tampered.evidence_commitment().unwrap();
        assert_eq!(
            verify_solana_stake_reader_proof_v1(&tampered, &reader_context(&tampered, &commitment)),
            Err(SolanaStakeError::CheckpointMismatch)
        );

        let mut tampered = proof;
        tampered.positions[0].delegated_lamports ^= 1;
        let commitment = tampered.evidence_commitment().unwrap();
        assert_eq!(
            verify_solana_stake_reader_proof_v1(&tampered, &reader_context(&tampered, &commitment)),
            Err(SolanaStakeError::CheckpointMismatch)
        );
    }

    #[test]
    fn reader_rejects_noncanonical_transaction_and_weak_or_stale_finality() {
        let proof = reader_fixture();
        assert_eq!(verify_reader_transaction(&proof), Ok(()));

        let signer = SigningKey::from_bytes(&[8; 32]);
        let mut noncanonical = proof.clone();
        noncanonical.transaction_message[2] -= 1;
        noncanonical.transaction_signature = signer
            .sign(&noncanonical.transaction_message)
            .to_bytes()
            .to_vec();
        assert_eq!(
            verify_reader_transaction(&noncanonical),
            Err(SolanaStakeError::Transaction)
        );

        let mut weak = proof.clone();
        weak.checkpoint_certificate.checkpoint.minimum_depth = 1;
        resign_reader_checkpoint_and_owner(&mut weak);
        let commitment = weak.evidence_commitment().unwrap();
        assert_eq!(
            verify_solana_stake_reader_proof_v1(&weak, &reader_context(&weak, &commitment)),
            Err(SolanaStakeError::CheckpointMismatch)
        );

        let mut stale = proof;
        stale.checkpoint_certificate.checkpoint.observed_source_head = 590;
        resign_reader_checkpoint_and_owner(&mut stale);
        let commitment = stale.evidence_commitment().unwrap();
        assert_eq!(
            verify_solana_stake_reader_proof_v1(&stale, &reader_context(&stale, &commitment)),
            Err(SolanaStakeError::CheckpointMismatch)
        );
    }

    #[test]
    fn registered_guest_dispatch_executes_reader_proof_as_cryptographic() {
        use crate::{
            verify_observation_evidence, EvidenceDimensionV1, FreshnessPolicyV1,
            LiabilityTreatmentV1, ReserveProofContextV1, SourceEvidenceV1, SourceManifestEntryV1,
            SourceObservationV1, TrustClassV1,
        };

        let proof = reader_fixture();
        let evidence_commitment = proof.evidence_commitment().unwrap();
        let context = ReserveProofContextV1 {
            pftl_genesis_hash: "11".repeat(48),
            nav_asset_id: "22".repeat(48),
            proof_profile_id: "33".repeat(48),
            valuation_policy_hash: "44".repeat(32),
            source_manifest_hash: "55".repeat(48),
            valuation_unit_id: "66".repeat(48),
            valuation_scale: 100_000_000,
            observation_epoch: 1,
            observation_not_before: 500,
            observation_not_after: 500,
        };
        let entry = SourceManifestEntryV1 {
            source_id: "solana-stake-primary".to_string(),
            adapter_kind: SOLANA_STAKE_READER_ADAPTER_KIND_V1.to_string(),
            source_domain: proof.policy.source_domain.clone(),
            asset_or_position_id: proof.policy.position_set_id.clone(),
            reserve_owner_commitment: solana_stake_owner_commitment(proof.policy.wallet_pubkey),
            quantity_verifier_commitment: proof.policy.commitment().unwrap(),
            valuation_verifier_commitment: "77".repeat(48),
            quantity_evidence_class: TrustClassV1::Cryptographic,
            valuation_evidence_class: TrustClassV1::Controlled,
            freshness_policy: FreshnessPolicyV1 {
                max_age_blocks: 10,
                max_observation_span_blocks: 10,
            },
            haircut_policy_hash: "88".repeat(48),
            liability_treatment: LiabilityTreatmentV1::Asset,
            adapter_schema_version: 1,
        };
        let observation = SourceObservationV1 {
            source_id: entry.source_id.clone(),
            observed_at_block: 500,
            gross_assets: 1,
            total_liabilities: 0,
            quantity_evidence: SourceEvidenceV1::SolanaStakeReader {
                evidence_commitment,
                proof: Box::new(proof),
            },
            valuation_evidence: SourceEvidenceV1::Controlled {
                evidence_commitment: "99".repeat(48),
            },
            disclosure_commitment: "aa".repeat(48),
        };
        assert_eq!(
            observation.quantity_evidence.class(),
            TrustClassV1::Cryptographic
        );
        verify_observation_evidence(
            &context,
            &entry,
            &observation,
            EvidenceDimensionV1::Quantity,
        )
        .unwrap();
    }

    #[test]
    fn registered_guest_dispatch_preserves_attested_quantity_class() {
        use crate::{
            execute_reserve_proof, FreshnessPolicyV1, LiabilityTreatmentV1, ReserveProofContextV1,
            ReserveProofWitnessV1, SourceEvidenceV1, SourceManifestEntryV1, SourceManifestV1,
            SourceObservationV1, TrustClassV1, MANIFEST_SCHEMA_V1, WITNESS_SCHEMA_V1,
        };

        let (mut proof, owner, keys) = fixture();
        let manifest = SourceManifestV1 {
            schema: MANIFEST_SCHEMA_V1.to_string(),
            sources: vec![SourceManifestEntryV1 {
                source_id: "solana-stake-primary".to_string(),
                adapter_kind: SOLANA_STAKE_ADAPTER_KIND_V1.to_string(),
                source_domain: proof.policy.source_domain.clone(),
                asset_or_position_id: proof.policy.position_set_id.clone(),
                reserve_owner_commitment: solana_stake_owner_commitment(proof.policy.wallet_pubkey),
                quantity_verifier_commitment: proof.policy.commitment().unwrap(),
                valuation_verifier_commitment: "66".repeat(48),
                quantity_evidence_class: TrustClassV1::Attested,
                valuation_evidence_class: TrustClassV1::Controlled,
                freshness_policy: FreshnessPolicyV1 {
                    max_age_blocks: 10,
                    max_observation_span_blocks: 10,
                },
                haircut_policy_hash: "77".repeat(48),
                liability_treatment: LiabilityTreatmentV1::Asset,
                adapter_schema_version: 1,
            }],
        };
        let reserve_context = ReserveProofContextV1 {
            pftl_genesis_hash: "11".repeat(48),
            nav_asset_id: "22".repeat(48),
            proof_profile_id: "33".repeat(48),
            valuation_policy_hash: "44".repeat(32),
            source_manifest_hash: manifest.hash().unwrap(),
            valuation_unit_id: "88".repeat(48),
            valuation_scale: 100_000_000,
            observation_epoch: 1,
            observation_not_before: 500,
            observation_not_after: 500,
        };
        let verify_context = SolanaStakeVerifyContextV1 {
            pftl_genesis_hash: &reserve_context.pftl_genesis_hash,
            nav_asset_id: &reserve_context.nav_asset_id,
            proof_profile_id: &reserve_context.proof_profile_id,
            valuation_policy_hash: &reserve_context.valuation_policy_hash,
            source_manifest_hash: &reserve_context.source_manifest_hash,
            source_id: &manifest.sources[0].source_id,
            source_domain: &manifest.sources[0].source_domain,
            asset_or_position_id: &manifest.sources[0].asset_or_position_id,
            reserve_owner_commitment: &manifest.sources[0].reserve_owner_commitment,
            quantity_verifier_commitment: &manifest.sources[0].quantity_verifier_commitment,
            observed_at_pftl_height: 500,
            expected_evidence_commitment: "00",
        };
        let statements = proof
            .attestations
            .iter()
            .map(|attestation| attestation_statement(&proof, attestation, &verify_context).unwrap())
            .collect::<Vec<_>>();
        for ((attestation, key), statement) in
            proof.attestations.iter_mut().zip(&keys).zip(statements)
        {
            attestation.signature = key.sign(&statement).to_bytes().to_vec();
        }
        proof.ownership_signature = owner
            .sign(&owner_statement(&proof, &verify_context).unwrap())
            .to_bytes()
            .to_vec();
        let evidence_commitment = proof.evidence_commitment().unwrap();
        let witness = ReserveProofWitnessV1 {
            schema: WITNESS_SCHEMA_V1.to_string(),
            context: reserve_context,
            manifest,
            observations: vec![SourceObservationV1 {
                source_id: "solana-stake-primary".to_string(),
                observed_at_block: 500,
                gross_assets: 123,
                total_liabilities: 0,
                quantity_evidence: SourceEvidenceV1::SolanaStakeAttested {
                    evidence_commitment,
                    proof: Box::new(proof),
                },
                valuation_evidence: SourceEvidenceV1::Controlled {
                    evidence_commitment: "99".repeat(48),
                },
                disclosure_commitment: "aa".repeat(48),
            }],
        };
        let public = execute_reserve_proof(&witness).unwrap();
        assert_eq!(public.quantity_trust_counts.attested, 1);
        assert_eq!(public.quantity_trust_counts.cryptographic, 0);
    }

    #[test]
    fn rejects_disagreement_bad_authority_duplicate_and_self_attestation() {
        let (mut proof, owner, keys) = fixture();
        authorize(&mut proof, &owner, &keys);
        let mut disagreement = proof.clone();
        disagreement.attestations[2].finalized_slot += 1;
        authorize(&mut disagreement, &owner, &keys);
        let commitment = disagreement.evidence_commitment().unwrap();
        assert_eq!(
            verify_solana_stake_attested_proof_v1(
                &disagreement,
                &context(&disagreement, &commitment)
            ),
            Err(SolanaStakeError::AttestationDisagreement)
        );

        let mut authority = proof.clone();
        authority.attestations.iter_mut().for_each(|attestation| {
            attestation.stake_accounts[0].data[44] ^= 1;
        });
        authorize(&mut authority, &owner, &keys);
        let commitment = authority.evidence_commitment().unwrap();
        assert_eq!(
            verify_solana_stake_attested_proof_v1(&authority, &context(&authority, &commitment)),
            Err(SolanaStakeError::StakeState)
        );

        let mut duplicate = proof.policy.clone();
        duplicate.positions.push(duplicate.positions[0].clone());
        assert!(duplicate.validate().is_err());

        let mut self_attested = proof.policy;
        self_attested.attestors[0].kind = SolanaAttestorKindV1::SelfAttested;
        assert_eq!(
            self_attested.validate(),
            Err(SolanaStakeError::InsufficientAttestations)
        );
    }

    #[derive(Deserialize)]
    struct HistoricalWitness {
        wallet_pubkey: [u8; 32],
        attestations: Vec<HistoricalAttestation>,
    }

    #[derive(Deserialize)]
    struct HistoricalAttestation {
        clock_sysvar_data: Vec<u8>,
        stake_accounts: Vec<SolanaStakeAccountSnapshotV1>,
    }

    #[test]
    fn historical_a666_solana_artifact_reconstructs_quantities_and_authorities() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../benchmarks/nav-reserve-proof-historical/solana-attested-witness.json"
        );
        let historical: HistoricalWitness =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let canonical = &historical.attestations[0];
        let epoch = parse_clock_epoch(&canonical.clock_sysvar_data).unwrap();
        let mut total = 0u64;
        for snapshot in &canonical.stake_accounts {
            if snapshot.exists {
                assert_eq!(
                    read_u32_le(&snapshot.data, 0).unwrap(),
                    STAKE_STATE_V2_DELEGATED
                );
                assert_eq!(
                    read_pubkey(&snapshot.data, 44).unwrap(),
                    historical.wallet_pubkey
                );
                total += snapshot.lamports;
            }
        }
        assert_eq!(total, 14_499_000_000);
        assert!(epoch > 0);
    }
}
