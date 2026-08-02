//! Complete, provider-neutral EVM spot quantity verification beneath governed
//! quorum-certified state roots.
//!
//! Reserve quantities are proven cryptographically. USD prices remain a
//! separate manifest trust dimension and are never promoted by this adapter.

use alloy_primitives::{keccak256, Address, Bytes, Signature, B256, U256};
use alloy_rlp::{encode, encode_fixed_size};
use alloy_trie::{nybbles::Nibbles, proof::verify_proof, TrieAccount};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};

use crate::bft_checkpoint::{BftSourceCheckpointCertificateV1, BftSourceCheckpointV1};
use crate::evm_checkpoint::{EvmAccountProofV1, EvmStorageProofV1};

pub const EVM_SPOT_ADAPTER_KIND_V1: &str = "evm-spot-set-bft-checkpoint-mpt-v1";
pub const EVM_SPOT_CHECKPOINT_KIND_V1: &str = "evm-state-root-v1";
pub const MAX_EVM_SPOT_CHAINS: usize = 16;
pub const MAX_EVM_SPOT_TOKENS_PER_CHAIN: usize = 64;
pub const MAX_EVM_SPOT_TOTAL_POSITIONS: usize = 256;
pub const MAX_EVM_SPOT_PROOF_NODES: usize = 64;
pub const MAX_EVM_SPOT_PROOF_NODE_BYTES: usize = 64 * 1024;
pub const MAX_EVM_SPOT_PROOF_TOTAL_BYTES: usize = 2 * 1024 * 1024;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_evm_spot_policy.v1";
const OWNER_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_evm_spot_owner.v1";
const OWNER_AUTHORIZATION_DOMAIN: &[u8] = b"postfiat.reserve_evm_spot_owner_authorization.v1";
const EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_evm_spot_evidence.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmSpotTokenPolicyV1 {
    pub position_id: String,
    pub token: Address,
    pub token_code_hash: B256,
    pub balance_slot_index: U256,
    pub decimals: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmSpotChainPolicyV1 {
    pub chain_id: u64,
    pub source_domain: String,
    pub committee_root: String,
    pub native_position_id: String,
    pub native_account_code_hash: B256,
    pub native_decimals: u8,
    pub tokens: Vec<EvmSpotTokenPolicyV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmSpotPolicyV1 {
    pub aggregate_source_domain: String,
    pub aggregate_position_id: String,
    pub maximum_timestamp_skew_ms: u64,
    pub chains: Vec<EvmSpotChainPolicyV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmSpotTokenProofV1 {
    pub position_id: String,
    pub token_account: EvmAccountProofV1,
    pub balance: EvmStorageProofV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmSpotChainProofV1 {
    pub checkpoint_certificate: BftSourceCheckpointCertificateV1,
    pub native_account: EvmAccountProofV1,
    pub tokens: Vec<EvmSpotTokenProofV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmSpotQuantityProofV1 {
    pub policy: EvmSpotPolicyV1,
    pub owner: Address,
    pub ownership_signature: Vec<u8>,
    pub chains: Vec<EvmSpotChainProofV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvmSpotQuantityRowV1 {
    pub position_id: String,
    pub chain_id: u64,
    pub decimals: u8,
    pub raw_quantity: U256,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvmSpotQuantityVerificationV1 {
    pub minimum_source_timestamp_ms: u64,
    pub maximum_source_timestamp_ms: u64,
    pub rows: Vec<EvmSpotQuantityRowV1>,
    pub evidence_commitment: String,
}

pub struct EvmSpotVerifyContextV1<'a> {
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
pub enum EvmSpotError {
    BoundsExceeded,
    PolicyMismatch,
    CheckpointMismatch,
    TimestampMismatch,
    OwnerAuthorization,
    EvidenceCommitment,
    PositionMismatch,
    AccountProof,
    StorageProof,
    ArithmeticOverflow,
}

impl EvmSpotPolicyV1 {
    pub fn validate(&self) -> Result<(), EvmSpotError> {
        validate_identifier(&self.aggregate_source_domain)?;
        validate_identifier(&self.aggregate_position_id)?;
        if self.maximum_timestamp_skew_ms == 0
            || self.chains.is_empty()
            || self.chains.len() > MAX_EVM_SPOT_CHAINS
        {
            return Err(EvmSpotError::PolicyMismatch);
        }
        let mut previous_chain = 0u64;
        let mut total_positions = 0usize;
        for chain in &self.chains {
            validate_identifier(&chain.source_domain)?;
            validate_identifier(&chain.native_position_id)?;
            validate_lower_hex(&chain.committee_root, 48)?;
            if chain.chain_id == 0
                || chain.chain_id <= previous_chain
                || chain.source_domain != format!("eip155:{}", chain.chain_id)
                || chain.native_account_code_hash == B256::ZERO
                || chain.native_decimals > 38
                || chain.tokens.len() > MAX_EVM_SPOT_TOKENS_PER_CHAIN
            {
                return Err(EvmSpotError::PolicyMismatch);
            }
            previous_chain = chain.chain_id;
            total_positions = total_positions
                .checked_add(1 + chain.tokens.len())
                .ok_or(EvmSpotError::ArithmeticOverflow)?;
            let mut previous_position: Option<&str> = None;
            let mut previous_token = Address::ZERO;
            for token in &chain.tokens {
                validate_identifier(&token.position_id)?;
                if previous_position >= Some(token.position_id.as_str())
                    || token.position_id == chain.native_position_id
                    || token.token == Address::ZERO
                    || token.token <= previous_token
                    || token.token_code_hash == B256::ZERO
                    || token.decimals > 38
                {
                    return Err(EvmSpotError::PolicyMismatch);
                }
                previous_position = Some(token.position_id.as_str());
                previous_token = token.token;
            }
        }
        if total_positions > MAX_EVM_SPOT_TOTAL_POSITIONS {
            return Err(EvmSpotError::BoundsExceeded);
        }
        Ok(())
    }

    pub fn commitment(&self) -> Result<String, EvmSpotError> {
        self.validate()?;
        let mut out = Vec::new();
        append_bytes(&mut out, self.aggregate_source_domain.as_bytes())?;
        append_bytes(&mut out, self.aggregate_position_id.as_bytes())?;
        out.extend_from_slice(&self.maximum_timestamp_skew_ms.to_be_bytes());
        append_u32(&mut out, self.chains.len())?;
        for chain in &self.chains {
            out.extend_from_slice(&chain.chain_id.to_be_bytes());
            append_bytes(&mut out, chain.source_domain.as_bytes())?;
            append_hex(&mut out, &chain.committee_root, 48)?;
            append_bytes(&mut out, chain.native_position_id.as_bytes())?;
            out.extend_from_slice(chain.native_account_code_hash.as_slice());
            out.push(chain.native_decimals);
            append_u32(&mut out, chain.tokens.len())?;
            for token in &chain.tokens {
                append_bytes(&mut out, token.position_id.as_bytes())?;
                out.extend_from_slice(token.token.as_slice());
                out.extend_from_slice(token.token_code_hash.as_slice());
                out.extend_from_slice(&token.balance_slot_index.to_be_bytes::<32>());
                out.push(token.decimals);
            }
        }
        Ok(hash48(POLICY_COMMITMENT_DOMAIN, &[&out]))
    }
}

pub fn evm_spot_owner_commitment(owner: Address) -> String {
    hash48(OWNER_COMMITMENT_DOMAIN, &[owner.as_slice()])
}

pub fn verify_evm_spot_quantity_proof_v1(
    proof: &EvmSpotQuantityProofV1,
    context: &EvmSpotVerifyContextV1<'_>,
) -> Result<EvmSpotQuantityVerificationV1, EvmSpotError> {
    proof.policy.validate()?;
    validate_proof_bounds(proof)?;
    if proof.policy.aggregate_source_domain != context.source_domain
        || proof.policy.aggregate_position_id != context.asset_or_position_id
        || evm_spot_owner_commitment(proof.owner) != context.reserve_owner_commitment
        || proof.policy.commitment()? != context.quantity_verifier_commitment
        || proof.chains.len() != proof.policy.chains.len()
    {
        return Err(EvmSpotError::PolicyMismatch);
    }
    let computed_commitment = proof.evidence_commitment()?;
    if computed_commitment != context.expected_evidence_commitment {
        return Err(EvmSpotError::EvidenceCommitment);
    }
    verify_owner_authorization(proof, context)?;

    let mut rows = Vec::new();
    let mut minimum_timestamp = u64::MAX;
    let mut maximum_timestamp = 0u64;
    for (chain_policy, chain_proof) in proof.policy.chains.iter().zip(&proof.chains) {
        verify_checkpoint(chain_policy, chain_proof, context)?;
        let checkpoint = &chain_proof.checkpoint_certificate.checkpoint;
        minimum_timestamp = minimum_timestamp.min(checkpoint.source_timestamp_ms);
        maximum_timestamp = maximum_timestamp.max(checkpoint.source_timestamp_ms);
        if chain_proof.native_account.address != proof.owner
            || chain_proof.native_account.code_hash != chain_policy.native_account_code_hash
        {
            return Err(EvmSpotError::PositionMismatch);
        }
        verify_account_proof(
            checkpoint.source_state_commitment,
            &chain_proof.native_account,
        )?;
        rows.push(EvmSpotQuantityRowV1 {
            position_id: chain_policy.native_position_id.clone(),
            chain_id: chain_policy.chain_id,
            decimals: chain_policy.native_decimals,
            raw_quantity: chain_proof.native_account.balance,
        });
        if chain_proof.tokens.len() != chain_policy.tokens.len() {
            return Err(EvmSpotError::PositionMismatch);
        }
        for (token_policy, token_proof) in chain_policy.tokens.iter().zip(&chain_proof.tokens) {
            if token_proof.position_id != token_policy.position_id
                || token_proof.token_account.address != token_policy.token
                || token_proof.token_account.code_hash != token_policy.token_code_hash
            {
                return Err(EvmSpotError::PositionMismatch);
            }
            verify_account_proof(
                checkpoint.source_state_commitment,
                &token_proof.token_account,
            )?;
            let expected_slot = erc20_balance_slot(proof.owner, token_policy.balance_slot_index);
            if token_proof.balance.key != expected_slot {
                return Err(EvmSpotError::PositionMismatch);
            }
            verify_storage_proof(token_proof.token_account.storage_root, &token_proof.balance)?;
            rows.push(EvmSpotQuantityRowV1 {
                position_id: token_policy.position_id.clone(),
                chain_id: chain_policy.chain_id,
                decimals: token_policy.decimals,
                raw_quantity: token_proof.balance.value,
            });
        }
    }
    if maximum_timestamp
        .checked_sub(minimum_timestamp)
        .ok_or(EvmSpotError::TimestampMismatch)?
        > proof.policy.maximum_timestamp_skew_ms
    {
        return Err(EvmSpotError::TimestampMismatch);
    }
    Ok(EvmSpotQuantityVerificationV1 {
        minimum_source_timestamp_ms: minimum_timestamp,
        maximum_source_timestamp_ms: maximum_timestamp,
        rows,
        evidence_commitment: computed_commitment,
    })
}

impl EvmSpotQuantityProofV1 {
    pub fn evidence_commitment(&self) -> Result<String, EvmSpotError> {
        self.policy.validate()?;
        validate_proof_bounds(self)?;
        let mut out = Vec::new();
        append_hex(&mut out, &self.policy.commitment()?, 48)?;
        out.extend_from_slice(self.owner.as_slice());
        append_bytes(&mut out, &self.ownership_signature)?;
        append_u32(&mut out, self.chains.len())?;
        for chain in &self.chains {
            append_checkpoint(&mut out, &chain.checkpoint_certificate.checkpoint)?;
            append_u32(&mut out, chain.checkpoint_certificate.votes.len())?;
            for vote in &chain.checkpoint_certificate.votes {
                append_bytes(&mut out, vote.validator_id.as_bytes())?;
                append_bytes(&mut out, &vote.signature)?;
            }
            append_account_proof(&mut out, &chain.native_account)?;
            append_u32(&mut out, chain.tokens.len())?;
            for token in &chain.tokens {
                append_bytes(&mut out, token.position_id.as_bytes())?;
                append_account_proof(&mut out, &token.token_account)?;
                append_storage_proof(&mut out, &token.balance)?;
            }
        }
        Ok(hash48(EVIDENCE_COMMITMENT_DOMAIN, &[&out]))
    }
}

fn verify_checkpoint(
    policy: &EvmSpotChainPolicyV1,
    proof: &EvmSpotChainProofV1,
    context: &EvmSpotVerifyContextV1<'_>,
) -> Result<(), EvmSpotError> {
    proof
        .checkpoint_certificate
        .verify()
        .map_err(|_| EvmSpotError::CheckpointMismatch)?;
    let checkpoint = &proof.checkpoint_certificate.checkpoint;
    if checkpoint.pftl_genesis_hash != context.pftl_genesis_hash
        || checkpoint.checkpoint_kind != EVM_SPOT_CHECKPOINT_KIND_V1
        || checkpoint.source_domain != policy.source_domain
        || checkpoint.pftl_observation_height != context.observed_at_pftl_height
        || checkpoint.committee_root != policy.committee_root
        || checkpoint.source_state_commitment == B256::ZERO
    {
        return Err(EvmSpotError::CheckpointMismatch);
    }
    Ok(())
}

fn verify_owner_authorization(
    proof: &EvmSpotQuantityProofV1,
    context: &EvmSpotVerifyContextV1<'_>,
) -> Result<(), EvmSpotError> {
    let signature: [u8; 65] = proof
        .ownership_signature
        .as_slice()
        .try_into()
        .map_err(|_| EvmSpotError::OwnerAuthorization)?;
    let signature =
        Signature::from_raw_array(&signature).map_err(|_| EvmSpotError::OwnerAuthorization)?;
    let statement = owner_authorization_statement(proof, context)?;
    let recovered = signature
        .recover_address_from_msg(&statement)
        .map_err(|_| EvmSpotError::OwnerAuthorization)?;
    if recovered != proof.owner {
        return Err(EvmSpotError::OwnerAuthorization);
    }
    Ok(())
}

fn owner_authorization_statement(
    proof: &EvmSpotQuantityProofV1,
    context: &EvmSpotVerifyContextV1<'_>,
) -> Result<Vec<u8>, EvmSpotError> {
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
    append_hex(&mut out, &proof.policy.commitment()?, 48)?;
    out.extend_from_slice(proof.owner.as_slice());
    append_u32(&mut out, proof.chains.len())?;
    for chain in &proof.chains {
        append_checkpoint(&mut out, &chain.checkpoint_certificate.checkpoint)?;
    }
    Ok(domain_message(OWNER_AUTHORIZATION_DOMAIN, &out))
}

pub fn erc20_balance_slot(owner: Address, slot_index: U256) -> B256 {
    let mut encoded = [0u8; 64];
    encoded[12..32].copy_from_slice(owner.as_slice());
    encoded[32..].copy_from_slice(&slot_index.to_be_bytes::<32>());
    keccak256(encoded)
}

fn verify_account_proof(root: B256, account: &EvmAccountProofV1) -> Result<(), EvmSpotError> {
    validate_nodes(&account.proof)?;
    let proof = account
        .proof
        .iter()
        .cloned()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    let expected = encode(TrieAccount {
        nonce: account.nonce,
        balance: account.balance,
        storage_root: account.storage_root,
        code_hash: account.code_hash,
    });
    verify_proof(
        root,
        Nibbles::unpack(keccak256(account.address.as_slice())),
        Some(expected),
        proof.iter(),
    )
    .map_err(|_| EvmSpotError::AccountProof)
}

fn verify_storage_proof(root: B256, slot: &EvmStorageProofV1) -> Result<(), EvmSpotError> {
    validate_nodes(&slot.proof)?;
    let proof = slot
        .proof
        .iter()
        .cloned()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    let expected = if slot.value == U256::ZERO {
        None
    } else {
        Some(encode_fixed_size(&slot.value).as_ref().to_vec())
    };
    verify_proof(
        root,
        Nibbles::unpack(keccak256(slot.key.as_slice())),
        expected,
        proof.iter(),
    )
    .map_err(|_| EvmSpotError::StorageProof)
}

fn validate_proof_bounds(proof: &EvmSpotQuantityProofV1) -> Result<(), EvmSpotError> {
    if proof.ownership_signature.len() != 65 || proof.chains.len() > MAX_EVM_SPOT_CHAINS {
        return Err(EvmSpotError::BoundsExceeded);
    }
    let mut total = 0usize;
    for chain in &proof.chains {
        validate_nodes(&chain.native_account.proof)?;
        if chain.tokens.len() > MAX_EVM_SPOT_TOKENS_PER_CHAIN {
            return Err(EvmSpotError::BoundsExceeded);
        }
        for token in &chain.tokens {
            validate_identifier(&token.position_id)?;
            validate_nodes(&token.token_account.proof)?;
            validate_nodes(&token.balance.proof)?;
            total = total
                .checked_add(
                    token
                        .token_account
                        .proof
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>(),
                )
                .and_then(|value| {
                    value.checked_add(token.balance.proof.iter().map(Vec::len).sum::<usize>())
                })
                .ok_or(EvmSpotError::ArithmeticOverflow)?;
        }
        total = total
            .checked_add(
                chain
                    .native_account
                    .proof
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>(),
            )
            .ok_or(EvmSpotError::ArithmeticOverflow)?;
    }
    if total > MAX_EVM_SPOT_PROOF_TOTAL_BYTES {
        return Err(EvmSpotError::BoundsExceeded);
    }
    Ok(())
}

fn validate_nodes(nodes: &[Vec<u8>]) -> Result<(), EvmSpotError> {
    if nodes.is_empty() || nodes.len() > MAX_EVM_SPOT_PROOF_NODES {
        return Err(EvmSpotError::BoundsExceeded);
    }
    if nodes
        .iter()
        .any(|node| node.is_empty() || node.len() > MAX_EVM_SPOT_PROOF_NODE_BYTES)
    {
        return Err(EvmSpotError::BoundsExceeded);
    }
    Ok(())
}

fn append_checkpoint(
    out: &mut Vec<u8>,
    checkpoint: &BftSourceCheckpointV1,
) -> Result<(), EvmSpotError> {
    let bytes = checkpoint
        .canonical_bytes()
        .map_err(|_| EvmSpotError::CheckpointMismatch)?;
    append_bytes(out, &bytes)
}

fn append_account_proof(
    out: &mut Vec<u8>,
    account: &EvmAccountProofV1,
) -> Result<(), EvmSpotError> {
    out.extend_from_slice(account.address.as_slice());
    out.extend_from_slice(&account.nonce.to_be_bytes());
    out.extend_from_slice(&account.balance.to_be_bytes::<32>());
    out.extend_from_slice(account.storage_root.as_slice());
    out.extend_from_slice(account.code_hash.as_slice());
    append_nodes(out, &account.proof)
}

fn append_storage_proof(out: &mut Vec<u8>, slot: &EvmStorageProofV1) -> Result<(), EvmSpotError> {
    out.extend_from_slice(slot.key.as_slice());
    out.extend_from_slice(&slot.value.to_be_bytes::<32>());
    append_nodes(out, &slot.proof)
}

fn append_nodes(out: &mut Vec<u8>, nodes: &[Vec<u8>]) -> Result<(), EvmSpotError> {
    validate_nodes(nodes)?;
    append_u32(out, nodes.len())?;
    for node in nodes {
        append_bytes(out, node)?;
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), EvmSpotError> {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(EvmSpotError::PolicyMismatch);
    }
    Ok(())
}

fn validate_lower_hex(value: &str, bytes: usize) -> Result<(), EvmSpotError> {
    if value.len() != bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EvmSpotError::PolicyMismatch);
    }
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), EvmSpotError> {
    validate_lower_hex(value, bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| EvmSpotError::PolicyMismatch)?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), EvmSpotError> {
    let length = u32::try_from(value.len()).map_err(|_| EvmSpotError::BoundsExceeded)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), EvmSpotError> {
    let value = u32::try_from(value).map_err(|_| EvmSpotError::BoundsExceeded)?;
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
    use crate::bft_checkpoint::{
        BftCheckpointCommitteeV1, BftCheckpointValidatorV1, BftSourceCheckpointVoteV1,
        BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
    };
    use alloy_primitives::eip191_hash_message;
    use alloy_trie::{nybbles::Nibbles, proof::ProofRetainer, HashBuilder};
    use k256::ecdsa::SigningKey;
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    fn committee() -> (BftCheckpointCommitteeV1, Vec<MlDsa65KeyPair>) {
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        let committee = BftCheckpointCommitteeV1 {
            epoch: 7,
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
        (committee, keys)
    }

    fn storage_proof(owner: Address, slot_index: U256, value: U256) -> (B256, EvmStorageProofV1) {
        let key = erc20_balance_slot(owner, slot_index);
        let path = Nibbles::unpack(keccak256(key.as_slice()));
        let mut builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([path]));
        if value != U256::ZERO {
            builder.add_leaf(path, encode_fixed_size(&value).as_ref());
        }
        let root = builder.root();
        let proof = builder
            .take_proof_nodes()
            .into_nodes_sorted()
            .into_iter()
            .map(|(_, node)| node.to_vec())
            .collect();
        (root, EvmStorageProofV1 { key, value, proof })
    }

    fn state_proofs(
        owner: Address,
        native_balance: U256,
        token: Address,
        token_code_hash: B256,
        storage_root: B256,
    ) -> (B256, EvmAccountProofV1, EvmAccountProofV1) {
        let native = TrieAccount {
            nonce: 2,
            balance: native_balance,
            storage_root: B256::repeat_byte(0x56),
            code_hash: B256::repeat_byte(0xc5),
        };
        let token_account = TrieAccount {
            nonce: 1,
            balance: U256::ZERO,
            storage_root,
            code_hash: token_code_hash,
        };
        let owner_path = Nibbles::unpack(keccak256(owner.as_slice()));
        let token_path = Nibbles::unpack(keccak256(token.as_slice()));
        let mut entries = vec![
            (owner_path, encode(native)),
            (token_path, encode(token_account)),
        ];
        entries.sort_by_key(|entry| entry.0);
        let mut builder = HashBuilder::default()
            .with_proof_retainer(ProofRetainer::from_iter([owner_path, token_path]));
        for (path, value) in entries {
            builder.add_leaf(path, &value);
        }
        let root = builder.root();
        let nodes = builder.take_proof_nodes();
        let proof_for = |path| {
            nodes
                .matching_nodes_sorted(&path)
                .into_iter()
                .map(|(_, node)| node.to_vec())
                .collect()
        };
        (
            root,
            EvmAccountProofV1 {
                address: owner,
                nonce: native.nonce,
                balance: native.balance,
                storage_root: native.storage_root,
                code_hash: native.code_hash,
                proof: proof_for(owner_path),
            },
            EvmAccountProofV1 {
                address: token,
                nonce: token_account.nonce,
                balance: token_account.balance,
                storage_root: token_account.storage_root,
                code_hash: token_account.code_hash,
                proof: proof_for(token_path),
            },
        )
    }

    fn certificate(
        chain_id: u64,
        state_root: B256,
        timestamp_ms: u64,
        committee: &BftCheckpointCommitteeV1,
        keys: &[MlDsa65KeyPair],
    ) -> BftSourceCheckpointCertificateV1 {
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: EVM_SPOT_CHECKPOINT_KIND_V1.to_string(),
            source_domain: format!("eip155:{chain_id}"),
            source_height: 100 + chain_id,
            source_timestamp_ms: timestamp_ms,
            source_block_hash: B256::repeat_byte(chain_id as u8),
            source_state_commitment: state_root,
            observed_source_head: 112 + chain_id,
            minimum_depth: 12,
            pftl_observation_height: 500,
            committee_epoch: committee.epoch,
            committee_root: committee.root().unwrap(),
        };
        let mut certificate = BftSourceCheckpointCertificateV1 {
            committee: committee.clone(),
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
                &[0x90 + index as u8; 32],
            )
            .unwrap();
        }
        certificate
    }

    fn fixture() -> (EvmSpotQuantityProofV1, SigningKey) {
        let owner_key = SigningKey::from_bytes((&[0x44; 32]).into()).unwrap();
        let owner = Address::from_private_key(&owner_key);
        let token = Address::repeat_byte(0x33);
        let code_hash = B256::repeat_byte(0x77);
        let slot_index = U256::from(9);
        let (storage_root, balance) = storage_proof(owner, slot_index, U256::from(123_456u64));
        let (state_root, native_account, token_account) =
            state_proofs(owner, U256::from(7_000u64), token, code_hash, storage_root);
        let (committee, keys) = committee();
        let committee_root = committee.root().unwrap();
        let policy = EvmSpotPolicyV1 {
            aggregate_source_domain: "evm-multichain:a666-v1".to_string(),
            aggregate_position_id: "evm-spot-set:a666-v1".to_string(),
            maximum_timestamp_skew_ms: 60_000,
            chains: vec![EvmSpotChainPolicyV1 {
                chain_id: 1,
                source_domain: "eip155:1".to_string(),
                committee_root,
                native_position_id: "ethereum-native-eth".to_string(),
                native_account_code_hash: native_account.code_hash,
                native_decimals: 18,
                tokens: vec![EvmSpotTokenPolicyV1 {
                    position_id: "ethereum-usdc".to_string(),
                    token,
                    token_code_hash: code_hash,
                    balance_slot_index: slot_index,
                    decimals: 6,
                }],
            }],
        };
        let proof = EvmSpotQuantityProofV1 {
            policy,
            owner,
            ownership_signature: vec![0; 65],
            chains: vec![EvmSpotChainProofV1 {
                checkpoint_certificate: certificate(
                    1,
                    state_root,
                    1_785_000_000_000,
                    &committee,
                    &keys,
                ),
                native_account,
                tokens: vec![EvmSpotTokenProofV1 {
                    position_id: "ethereum-usdc".to_string(),
                    token_account,
                    balance,
                }],
            }],
        };
        (proof, owner_key)
    }

    fn context<'a>(
        proof: &EvmSpotQuantityProofV1,
        commitment: &'a str,
    ) -> EvmSpotVerifyContextV1<'a> {
        EvmSpotVerifyContextV1 {
            pftl_genesis_hash: "11".repeat(48).leak(),
            nav_asset_id: "22".repeat(48).leak(),
            proof_profile_id: "33".repeat(48).leak(),
            valuation_policy_hash: "44".repeat(32).leak(),
            source_manifest_hash: "55".repeat(48).leak(),
            source_id: "evm-spot-primary",
            source_domain: "evm-multichain:a666-v1",
            asset_or_position_id: "evm-spot-set:a666-v1",
            reserve_owner_commitment: evm_spot_owner_commitment(proof.owner).leak(),
            quantity_verifier_commitment: proof.policy.commitment().unwrap().leak(),
            observed_at_pftl_height: 500,
            expected_evidence_commitment: commitment,
        }
    }

    fn authorize(proof: &mut EvmSpotQuantityProofV1, key: &SigningKey) {
        let context = context(proof, "00");
        let statement = owner_authorization_statement(proof, &context).unwrap();
        let digest = eip191_hash_message(&statement);
        let (signature, recovery_id) = key.sign_prehash_recoverable(digest.as_slice()).unwrap();
        proof.ownership_signature = Signature::from((signature, recovery_id))
            .as_bytes()
            .to_vec();
    }

    #[test]
    fn verifies_exact_spot_set_checkpoint_owner_and_state() {
        let (mut proof, key) = fixture();
        authorize(&mut proof, &key);
        let commitment = proof.evidence_commitment().unwrap();
        let verified =
            verify_evm_spot_quantity_proof_v1(&proof, &context(&proof, &commitment)).unwrap();
        assert_eq!(verified.rows.len(), 2);
        assert_eq!(verified.rows[0].raw_quantity, U256::from(7_000u64));
        assert_eq!(verified.rows[1].raw_quantity, U256::from(123_456u64));
    }

    #[test]
    fn registered_guest_dispatch_executes_complete_spot_quantity_verifier() {
        use crate::{
            execute_reserve_proof, FreshnessPolicyV1, LiabilityTreatmentV1, ReserveProofContextV1,
            ReserveProofWitnessV1, SourceEvidenceV1, SourceManifestEntryV1, SourceManifestV1,
            SourceObservationV1, TrustClassV1, MANIFEST_SCHEMA_V1, WITNESS_SCHEMA_V1,
        };

        let (mut proof, key) = fixture();
        let manifest = SourceManifestV1 {
            schema: MANIFEST_SCHEMA_V1.to_string(),
            sources: vec![SourceManifestEntryV1 {
                source_id: "evm-spot-primary".to_string(),
                adapter_kind: EVM_SPOT_ADAPTER_KIND_V1.to_string(),
                source_domain: proof.policy.aggregate_source_domain.clone(),
                asset_or_position_id: proof.policy.aggregate_position_id.clone(),
                reserve_owner_commitment: evm_spot_owner_commitment(proof.owner),
                quantity_verifier_commitment: proof.policy.commitment().unwrap(),
                valuation_verifier_commitment: "66".repeat(48),
                quantity_evidence_class: TrustClassV1::Cryptographic,
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
        let verify_context = EvmSpotVerifyContextV1 {
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
        let statement = owner_authorization_statement(&proof, &verify_context).unwrap();
        let digest = eip191_hash_message(&statement);
        let (signature, recovery_id) = key.sign_prehash_recoverable(digest.as_slice()).unwrap();
        proof.ownership_signature = Signature::from((signature, recovery_id))
            .as_bytes()
            .to_vec();
        let evidence_commitment = proof.evidence_commitment().unwrap();
        let witness = ReserveProofWitnessV1 {
            schema: WITNESS_SCHEMA_V1.to_string(),
            context: reserve_context,
            manifest,
            observations: vec![SourceObservationV1 {
                source_id: "evm-spot-primary".to_string(),
                observed_at_block: 500,
                gross_assets: 123,
                total_liabilities: 0,
                quantity_evidence: SourceEvidenceV1::EvmSpotQuantity {
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
        assert_eq!(public.gross_assets, 123);
        assert_eq!(public.quantity_trust_counts.cryptographic, 1);
        assert_eq!(public.valuation_trust_counts.controlled, 1);
    }

    #[test]
    fn rejects_omission_duplicate_owner_state_and_timestamp_substitution() {
        let (mut proof, key) = fixture();
        authorize(&mut proof, &key);

        let mut omitted = proof.clone();
        omitted.chains[0].tokens.clear();
        let omitted_commitment = omitted.evidence_commitment().unwrap();
        assert_eq!(
            verify_evm_spot_quantity_proof_v1(&omitted, &context(&omitted, &omitted_commitment)),
            Err(EvmSpotError::PositionMismatch)
        );

        let mut duplicate = proof.clone();
        let duplicate_token = duplicate.policy.chains[0].tokens[0].clone();
        duplicate.policy.chains[0].tokens.push(duplicate_token);
        assert!(duplicate.policy.validate().is_err());

        let mut bad_owner = proof.clone();
        bad_owner.owner = Address::repeat_byte(0x99);
        let bad_commitment = bad_owner.evidence_commitment().unwrap();
        assert_eq!(
            verify_evm_spot_quantity_proof_v1(&bad_owner, &context(&bad_owner, &bad_commitment)),
            Err(EvmSpotError::OwnerAuthorization)
        );

        let mut bad_state = proof.clone();
        bad_state.chains[0].tokens[0].balance.value += U256::from(1);
        let bad_commitment = bad_state.evidence_commitment().unwrap();
        assert_eq!(
            verify_evm_spot_quantity_proof_v1(&bad_state, &context(&bad_state, &bad_commitment)),
            Err(EvmSpotError::StorageProof)
        );

        let mut bad_timestamp = proof;
        bad_timestamp.chains[0]
            .checkpoint_certificate
            .checkpoint
            .source_timestamp_ms += 1;
        authorize(&mut bad_timestamp, &key);
        let bad_commitment = bad_timestamp.evidence_commitment().unwrap();
        assert_eq!(
            verify_evm_spot_quantity_proof_v1(
                &bad_timestamp,
                &context(&bad_timestamp, &bad_commitment)
            ),
            Err(EvmSpotError::CheckpointMismatch)
        );
    }

    #[derive(Deserialize)]
    struct HistoricalWitness {
        owner: Address,
        chains: Vec<HistoricalChain>,
    }

    #[derive(Deserialize)]
    struct HistoricalChain {
        chain_id: u64,
        state_root: B256,
        native_account: EvmAccountProofV1,
        erc20s: Vec<HistoricalToken>,
    }

    #[derive(Deserialize)]
    struct HistoricalToken {
        token: Address,
        balance_slot_index: U256,
        token_account: EvmAccountProofV1,
        balance: EvmStorageProofV1,
    }

    #[test]
    fn historical_a666_spot_artifact_reconstructs_all_state_proofs() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/evm-spot-witness.json"
        );
        let historical: HistoricalWitness =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(historical.chains.len(), 2);
        let mut token_count = 0usize;
        for chain in historical.chains {
            assert_eq!(chain.native_account.address, historical.owner);
            verify_account_proof(chain.state_root, &chain.native_account).unwrap();
            for token in chain.erc20s {
                assert_eq!(token.token_account.address, token.token);
                verify_account_proof(chain.state_root, &token.token_account).unwrap();
                assert_eq!(
                    token.balance.key,
                    erc20_balance_slot(historical.owner, token.balance_slot_index)
                );
                verify_storage_proof(token.token_account.storage_root, &token.balance).unwrap();
                token_count += 1;
            }
            assert!(matches!(chain.chain_id, 1 | 42_161));
        }
        assert_eq!(token_count, 2);
    }
}
