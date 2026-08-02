//! Public, provider-neutral verification of the governed Solana stake
//! attestation format.
//!
//! This adapter preserves the historical trust boundary: quantity is
//! attested, not cryptographic. It makes the attestors, exact stake-account
//! set, ownership, parser, slot, and stake state publicly auditable.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sha3::{Digest as Sha3Digest, Sha3_384};

pub const SOLANA_STAKE_ADAPTER_KIND_V1: &str = "solana-stake-attested-state-v1";
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
    use ed25519_dalek::{Signer, SigningKey};

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
            "/../../../../docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/solana-attested-witness.json"
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
