//! Public USD valuation from Chainlink state proofs beneath a governed EVM
//! checkpoint.
//!
//! This adapter does not accept a signed aggregate value. It consumes exact
//! quantities returned by a registered quantity verifier, verifies every
//! Chainlink feed from EVM account/storage Merkle proofs, applies the governed
//! haircut, and recomputes the observation's valuation-unit amount.

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};

use crate::aave_v3::{verify_chainlink_feed_proof_v1, ChainlinkFeedProofV1};
use crate::bft_checkpoint::BftSourceCheckpointCertificateV1;

pub const EVM_CHAINLINK_VALUATION_KIND_V1: &str = "evm-chainlink-state-proof-valuation-v1";
pub const EVM_STATE_CHECKPOINT_KIND_V1: &str = "evm-state-root-v1";
pub const MAX_VALUATION_ROWS: usize = 256;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_evm_chainlink_valuation_policy.v1";
const EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_evm_chainlink_valuation_evidence.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmChainlinkValuationRowPolicyV1 {
    pub position_id: String,
    pub quantity_decimals: u8,
    pub price_decimals: u8,
    pub haircut_bps: u16,
    pub proxy_address: Address,
    pub proxy_code_hash: B256,
    pub aggregator_code_hash: B256,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmChainlinkValuationPolicyV1 {
    pub source_domain: String,
    pub chain_id: u64,
    pub committee_root: String,
    pub valuation_policy_hash: String,
    pub valuation_unit_id: String,
    pub valuation_scale: u64,
    pub proxy_phase_slot_index: u64,
    pub hot_vars_slot_index: u64,
    pub transmissions_slot_index: u64,
    pub max_oracle_age_seconds: u64,
    pub rows: Vec<EvmChainlinkValuationRowPolicyV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvmChainlinkValuationProofV1 {
    pub policy: EvmChainlinkValuationPolicyV1,
    pub checkpoint_certificate: BftSourceCheckpointCertificateV1,
    pub quantity_evidence_commitment: String,
    pub feeds: Vec<ChainlinkFeedProofV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValuationQuantityRowV1 {
    pub position_id: String,
    pub raw_quantity: U256,
    pub decimals: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvmChainlinkValuationVerificationV1 {
    pub source_height: u64,
    pub source_block_hash: B256,
    pub source_state_root: B256,
    pub source_timestamp_seconds: u64,
    pub gross_assets: u64,
    pub evidence_commitment: String,
}

pub struct EvmChainlinkValuationVerifyContextV1<'a> {
    pub pftl_genesis_hash: &'a str,
    pub valuation_policy_hash: &'a str,
    pub valuation_unit_id: &'a str,
    pub valuation_scale: u64,
    pub observed_at_pftl_height: u64,
    pub valuation_verifier_commitment: &'a str,
    pub quantity_evidence_commitment: &'a str,
    pub expected_gross_assets: u64,
    pub expected_total_liabilities: u64,
    pub expected_evidence_commitment: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvmChainlinkValuationError {
    BoundsExceeded,
    PolicyMismatch,
    CheckpointMismatch,
    QuantityMismatch,
    OracleProof,
    ArithmeticOverflow,
    EvidenceCommitment,
}

impl EvmChainlinkValuationPolicyV1 {
    pub fn validate(&self) -> Result<(), EvmChainlinkValuationError> {
        validate_identifier(&self.source_domain)?;
        validate_hex(&self.committee_root, 48)?;
        validate_hex(&self.valuation_policy_hash, 32)?;
        validate_hex(&self.valuation_unit_id, 48)?;
        if self.chain_id == 0
            || self.source_domain != format!("eip155:{}", self.chain_id)
            || self.valuation_scale == 0
            || self.proxy_phase_slot_index == 0
            || self.hot_vars_slot_index == 0
            || self.transmissions_slot_index == 0
            || self.max_oracle_age_seconds == 0
            || self.rows.is_empty()
            || self.rows.len() > MAX_VALUATION_ROWS
        {
            return Err(EvmChainlinkValuationError::PolicyMismatch);
        }
        let mut previous: Option<&str> = None;
        for row in &self.rows {
            validate_identifier(&row.position_id)?;
            if previous >= Some(row.position_id.as_str())
                || row.quantity_decimals > 38
                || row.price_decimals > 38
                || row.haircut_bps == 0
                || row.haircut_bps > 10_000
                || row.proxy_address == Address::ZERO
                || row.proxy_code_hash == B256::ZERO
                || row.aggregator_code_hash == B256::ZERO
            {
                return Err(EvmChainlinkValuationError::PolicyMismatch);
            }
            previous = Some(&row.position_id);
        }
        Ok(())
    }

    pub fn commitment(&self) -> Result<String, EvmChainlinkValuationError> {
        self.validate()?;
        let mut out = Vec::new();
        append_bytes(&mut out, self.source_domain.as_bytes())?;
        out.extend_from_slice(&self.chain_id.to_be_bytes());
        append_hex(&mut out, &self.committee_root, 48)?;
        append_hex(&mut out, &self.valuation_policy_hash, 32)?;
        append_hex(&mut out, &self.valuation_unit_id, 48)?;
        out.extend_from_slice(&self.valuation_scale.to_be_bytes());
        for value in [
            self.proxy_phase_slot_index,
            self.hot_vars_slot_index,
            self.transmissions_slot_index,
            self.max_oracle_age_seconds,
        ] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        append_u32(&mut out, self.rows.len())?;
        for row in &self.rows {
            append_bytes(&mut out, row.position_id.as_bytes())?;
            out.push(row.quantity_decimals);
            out.push(row.price_decimals);
            out.extend_from_slice(&row.haircut_bps.to_be_bytes());
            out.extend_from_slice(row.proxy_address.as_slice());
            out.extend_from_slice(row.proxy_code_hash.as_slice());
            out.extend_from_slice(row.aggregator_code_hash.as_slice());
        }
        Ok(hash48(POLICY_COMMITMENT_DOMAIN, &[&out]))
    }
}

impl EvmChainlinkValuationProofV1 {
    pub fn evidence_commitment(&self) -> Result<String, EvmChainlinkValuationError> {
        self.policy.validate()?;
        validate_hex(&self.quantity_evidence_commitment, 48)?;
        if self.feeds.len() != self.policy.rows.len() {
            return Err(EvmChainlinkValuationError::BoundsExceeded);
        }
        let mut out = Vec::new();
        append_hex(&mut out, &self.policy.commitment()?, 48)?;
        append_hex(&mut out, &self.quantity_evidence_commitment, 48)?;
        append_bytes(
            &mut out,
            &self
                .checkpoint_certificate
                .checkpoint
                .canonical_bytes()
                .map_err(|_| EvmChainlinkValuationError::CheckpointMismatch)?,
        )?;
        append_u32(&mut out, self.checkpoint_certificate.votes.len())?;
        for vote in &self.checkpoint_certificate.votes {
            append_bytes(&mut out, vote.validator_id.as_bytes())?;
            append_bytes(&mut out, &vote.signature)?;
        }
        append_u32(&mut out, self.feeds.len())?;
        for feed in &self.feeds {
            append_account(&mut out, &feed.proxy_account)?;
            append_storage(&mut out, &feed.current_phase)?;
            append_account(&mut out, &feed.aggregator_account)?;
            append_storage(&mut out, &feed.hot_vars)?;
            append_storage(&mut out, &feed.transmission)?;
            out.push(feed.decimals);
        }
        Ok(hash48(EVIDENCE_COMMITMENT_DOMAIN, &[&out]))
    }
}

pub fn verify_evm_chainlink_valuation_v1(
    proof: &EvmChainlinkValuationProofV1,
    quantities: &[ValuationQuantityRowV1],
    context: &EvmChainlinkValuationVerifyContextV1<'_>,
) -> Result<EvmChainlinkValuationVerificationV1, EvmChainlinkValuationError> {
    proof.policy.validate()?;
    validate_hex(context.pftl_genesis_hash, 48)?;
    validate_hex(context.valuation_policy_hash, 32)?;
    validate_hex(context.valuation_unit_id, 48)?;
    validate_hex(context.valuation_verifier_commitment, 48)?;
    validate_hex(context.quantity_evidence_commitment, 48)?;
    validate_hex(context.expected_evidence_commitment, 48)?;
    if context.observed_at_pftl_height == 0
        || context.valuation_scale == 0
        || context.expected_total_liabilities != 0
        || proof.policy.valuation_policy_hash != context.valuation_policy_hash
        || proof.policy.valuation_unit_id != context.valuation_unit_id
        || proof.policy.valuation_scale != context.valuation_scale
        || proof.policy.commitment()? != context.valuation_verifier_commitment
        || proof.quantity_evidence_commitment != context.quantity_evidence_commitment
        || quantities.len() != proof.policy.rows.len()
        || proof.feeds.len() != proof.policy.rows.len()
    {
        return Err(EvmChainlinkValuationError::PolicyMismatch);
    }
    proof
        .checkpoint_certificate
        .verify()
        .map_err(|_| EvmChainlinkValuationError::CheckpointMismatch)?;
    let checkpoint = &proof.checkpoint_certificate.checkpoint;
    if checkpoint.pftl_genesis_hash != context.pftl_genesis_hash
        || checkpoint.checkpoint_kind != EVM_STATE_CHECKPOINT_KIND_V1
        || checkpoint.source_domain != proof.policy.source_domain
        || checkpoint.pftl_observation_height != context.observed_at_pftl_height
        || checkpoint.committee_root != proof.policy.committee_root
        || checkpoint.source_state_commitment == B256::ZERO
        || checkpoint.source_timestamp_ms == 0
        || checkpoint.source_timestamp_ms % 1_000 != 0
    {
        return Err(EvmChainlinkValuationError::CheckpointMismatch);
    }
    let block_timestamp = checkpoint.source_timestamp_ms / 1_000;
    let mut total = U256::ZERO;
    for ((policy, feed), quantity) in proof.policy.rows.iter().zip(&proof.feeds).zip(quantities) {
        if quantity.position_id != policy.position_id
            || quantity.decimals != policy.quantity_decimals
        {
            return Err(EvmChainlinkValuationError::QuantityMismatch);
        }
        let price = verify_chainlink_feed_proof_v1(
            feed,
            checkpoint.source_state_commitment,
            block_timestamp,
            policy.proxy_address,
            policy.proxy_code_hash,
            policy.aggregator_code_hash,
            proof.policy.proxy_phase_slot_index,
            proof.policy.hot_vars_slot_index,
            proof.policy.transmissions_slot_index,
            proof.policy.max_oracle_age_seconds,
            policy.price_decimals,
        )
        .map_err(|_| EvmChainlinkValuationError::OracleProof)?;
        let numerator = quantity
            .raw_quantity
            .checked_mul(U256::from(price))
            .and_then(|value| value.checked_mul(U256::from(context.valuation_scale)))
            .and_then(|value| value.checked_mul(U256::from(policy.haircut_bps)))
            .ok_or(EvmChainlinkValuationError::ArithmeticOverflow)?;
        let denominator = checked_pow10(policy.quantity_decimals)?
            .checked_mul(checked_pow10(policy.price_decimals)?)
            .and_then(|value| value.checked_mul(U256::from(10_000u64)))
            .ok_or(EvmChainlinkValuationError::ArithmeticOverflow)?;
        total = total
            .checked_add(numerator / denominator)
            .ok_or(EvmChainlinkValuationError::ArithmeticOverflow)?;
    }
    let gross_assets =
        u64::try_from(total).map_err(|_| EvmChainlinkValuationError::ArithmeticOverflow)?;
    if gross_assets != context.expected_gross_assets {
        return Err(EvmChainlinkValuationError::QuantityMismatch);
    }
    let evidence_commitment = proof.evidence_commitment()?;
    if evidence_commitment != context.expected_evidence_commitment {
        return Err(EvmChainlinkValuationError::EvidenceCommitment);
    }
    Ok(EvmChainlinkValuationVerificationV1 {
        source_height: checkpoint.source_height,
        source_block_hash: checkpoint.source_block_hash,
        source_state_root: checkpoint.source_state_commitment,
        source_timestamp_seconds: block_timestamp,
        gross_assets,
        evidence_commitment,
    })
}

fn checked_pow10(decimals: u8) -> Result<U256, EvmChainlinkValuationError> {
    let mut value = U256::from(1u8);
    for _ in 0..decimals {
        value = value
            .checked_mul(U256::from(10u8))
            .ok_or(EvmChainlinkValuationError::ArithmeticOverflow)?;
    }
    Ok(value)
}

fn append_account(
    out: &mut Vec<u8>,
    account: &crate::evm_checkpoint::EvmAccountProofV1,
) -> Result<(), EvmChainlinkValuationError> {
    out.extend_from_slice(account.address.as_slice());
    out.extend_from_slice(&account.nonce.to_be_bytes());
    out.extend_from_slice(&account.balance.to_be_bytes::<32>());
    out.extend_from_slice(account.storage_root.as_slice());
    out.extend_from_slice(account.code_hash.as_slice());
    append_nodes(out, &account.proof)
}

fn append_storage(
    out: &mut Vec<u8>,
    storage: &crate::evm_checkpoint::EvmStorageProofV1,
) -> Result<(), EvmChainlinkValuationError> {
    out.extend_from_slice(storage.key.as_slice());
    out.extend_from_slice(&storage.value.to_be_bytes::<32>());
    append_nodes(out, &storage.proof)
}

fn append_nodes(out: &mut Vec<u8>, nodes: &[Vec<u8>]) -> Result<(), EvmChainlinkValuationError> {
    append_u32(out, nodes.len())?;
    for node in nodes {
        append_bytes(out, node)?;
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), EvmChainlinkValuationError> {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(EvmChainlinkValuationError::PolicyMismatch);
    }
    Ok(())
}

fn validate_hex(value: &str, bytes: usize) -> Result<(), EvmChainlinkValuationError> {
    if value.len() != bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EvmChainlinkValuationError::PolicyMismatch);
    }
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), EvmChainlinkValuationError> {
    out.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| EvmChainlinkValuationError::BoundsExceeded)?
            .to_be_bytes(),
    );
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), EvmChainlinkValuationError> {
    out.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| EvmChainlinkValuationError::BoundsExceeded)?
            .to_be_bytes(),
    );
    Ok(())
}

fn append_hex(
    out: &mut Vec<u8>,
    value: &str,
    bytes: usize,
) -> Result<(), EvmChainlinkValuationError> {
    validate_hex(value, bytes)?;
    out.extend_from_slice(
        &hex::decode(value).map_err(|_| EvmChainlinkValuationError::PolicyMismatch)?,
    );
    Ok(())
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
pub(crate) mod tests {
    use super::*;
    use crate::aave_v3::{chainlink_transmission_slot, fixed_storage_slot};
    use crate::bft_checkpoint::{
        BftCheckpointCommitteeV1, BftCheckpointValidatorV1, BftSourceCheckpointV1,
        BftSourceCheckpointVoteV1, BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
    };
    use crate::evm_checkpoint::{EvmAccountProofV1, EvmStorageProofV1};
    use alloy_primitives::keccak256;
    use alloy_rlp::{encode, encode_fixed_size};
    use alloy_trie::{nybbles::Nibbles, proof::ProofRetainer, HashBuilder, TrieAccount};
    use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed};

    fn storage_root_and_proof(entries: &[(B256, U256)], target: B256) -> (B256, Vec<Vec<u8>>) {
        let mut leaves = entries
            .iter()
            .map(|(key, value)| {
                (
                    Nibbles::unpack(keccak256(key.as_slice())),
                    encode_fixed_size(value).as_ref().to_vec(),
                )
            })
            .collect::<Vec<_>>();
        leaves.sort_by_key(|left| left.0);
        let target_path = Nibbles::unpack(keccak256(target.as_slice()));
        let mut builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([target_path]));
        for (path, value) in leaves {
            builder.add_leaf(path, &value);
        }
        let root = builder.root();
        let proof = builder
            .take_proof_nodes()
            .into_nodes_sorted()
            .into_iter()
            .map(|(_, node)| node.to_vec())
            .collect();
        (root, proof)
    }

    fn account_root_and_proof(
        accounts: &[(Address, TrieAccount)],
        target: Address,
    ) -> (B256, Vec<Vec<u8>>) {
        let mut leaves = accounts
            .iter()
            .map(|(address, account)| {
                (
                    Nibbles::unpack(keccak256(address.as_slice())),
                    encode(*account),
                )
            })
            .collect::<Vec<_>>();
        leaves.sort_by_key(|left| left.0);
        let target_path = Nibbles::unpack(keccak256(target.as_slice()));
        let mut builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([target_path]));
        for (path, value) in leaves {
            builder.add_leaf(path, &value);
        }
        let root = builder.root();
        let proof = builder
            .take_proof_nodes()
            .into_nodes_sorted()
            .into_iter()
            .map(|(_, node)| node.to_vec())
            .collect();
        (root, proof)
    }

    pub(crate) fn fixture() -> (
        EvmChainlinkValuationProofV1,
        Vec<ValuationQuantityRowV1>,
        EvmChainlinkValuationVerifyContextV1<'static>,
    ) {
        let timestamp = 1_785_000_000u64;
        let proxy = Address::repeat_byte(0x21);
        let aggregator = Address::repeat_byte(0x22);
        let code_hash = B256::repeat_byte(0x33);
        let phase_key = fixed_storage_slot(2);
        let phase_value = (U256::from_be_slice(aggregator.as_slice()) << 16usize) | U256::from(1);
        let proxy_storage = vec![(phase_key, phase_value)];
        let (proxy_storage_root, _) = storage_root_and_proof(&proxy_storage, phase_key);
        let hot_key = fixed_storage_slot(13);
        let round = 7u32;
        let hot_value = U256::from(round) << 48usize;
        let transmission_key = chainlink_transmission_slot(round, 17);
        let transmission_value = (U256::from(timestamp - 5) << 224usize)
            | (U256::from(timestamp - 10) << 192usize)
            | U256::from(10_000_000_000u64);
        let aggregator_storage = vec![(hot_key, hot_value), (transmission_key, transmission_value)];
        let (aggregator_storage_root, _) = storage_root_and_proof(&aggregator_storage, hot_key);
        let accounts = vec![
            (
                proxy,
                TrieAccount {
                    nonce: 1,
                    balance: U256::ZERO,
                    storage_root: proxy_storage_root,
                    code_hash,
                },
            ),
            (
                aggregator,
                TrieAccount {
                    nonce: 1,
                    balance: U256::ZERO,
                    storage_root: aggregator_storage_root,
                    code_hash,
                },
            ),
        ];
        let (state_root, _) = account_root_and_proof(&accounts, proxy);
        let account = |address: Address, storage_root: B256| {
            let (_, proof) = account_root_and_proof(&accounts, address);
            EvmAccountProofV1 {
                address,
                nonce: 1,
                balance: U256::ZERO,
                storage_root,
                code_hash,
                proof,
            }
        };
        let storage = |entries: &[(B256, U256)], key: B256| {
            let (_, proof) = storage_root_and_proof(entries, key);
            EvmStorageProofV1 {
                key,
                value: entries
                    .iter()
                    .find(|(candidate, _)| *candidate == key)
                    .unwrap()
                    .1,
                proof,
            }
        };
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        let committee = BftCheckpointCommitteeV1 {
            epoch: 8,
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
        let policy = EvmChainlinkValuationPolicyV1 {
            source_domain: "eip155:1".to_string(),
            chain_id: 1,
            committee_root: committee.root().unwrap(),
            valuation_policy_hash: "77".repeat(32),
            valuation_unit_id: "55".repeat(48),
            valuation_scale: 100_000_000,
            proxy_phase_slot_index: 2,
            hot_vars_slot_index: 13,
            transmissions_slot_index: 17,
            max_oracle_age_seconds: 60,
            rows: vec![EvmChainlinkValuationRowPolicyV1 {
                position_id: "sol-stake".to_string(),
                quantity_decimals: 9,
                price_decimals: 8,
                haircut_bps: 9_000,
                proxy_address: proxy,
                proxy_code_hash: code_hash,
                aggregator_code_hash: code_hash,
            }],
        };
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: EVM_STATE_CHECKPOINT_KIND_V1.to_string(),
            source_domain: policy.source_domain.clone(),
            source_height: 20_000_000,
            source_timestamp_ms: timestamp * 1_000,
            source_block_hash: B256::repeat_byte(0x44),
            source_state_commitment: state_root,
            observed_source_head: 20_000_012,
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
        let proof = EvmChainlinkValuationProofV1 {
            policy,
            checkpoint_certificate: certificate,
            quantity_evidence_commitment: "66".repeat(48),
            feeds: vec![ChainlinkFeedProofV1 {
                proxy_account: account(proxy, proxy_storage_root),
                current_phase: storage(&proxy_storage, phase_key),
                aggregator_account: account(aggregator, aggregator_storage_root),
                hot_vars: storage(&aggregator_storage, hot_key),
                transmission: storage(&aggregator_storage, transmission_key),
                decimals: 8,
            }],
        };
        let evidence = proof.evidence_commitment().unwrap();
        let policy_commitment = proof.policy.commitment().unwrap();
        let context = EvmChainlinkValuationVerifyContextV1 {
            pftl_genesis_hash: Box::leak("11".repeat(48).into_boxed_str()),
            valuation_policy_hash: Box::leak("77".repeat(32).into_boxed_str()),
            valuation_unit_id: Box::leak("55".repeat(48).into_boxed_str()),
            valuation_scale: 100_000_000,
            observed_at_pftl_height: 500,
            valuation_verifier_commitment: Box::leak(policy_commitment.into_boxed_str()),
            quantity_evidence_commitment: Box::leak("66".repeat(48).into_boxed_str()),
            expected_gross_assets: 18_000_000_000,
            expected_total_liabilities: 0,
            expected_evidence_commitment: Box::leak(evidence.into_boxed_str()),
        };
        let quantities = vec![ValuationQuantityRowV1 {
            position_id: "sol-stake".to_string(),
            raw_quantity: U256::from(2_000_000_000u64),
            decimals: 9,
        }];
        // Keep the fixture mutable during construction to make accidental
        // future additions explicit at this point.
        proof.policy.validate().unwrap();
        (proof, quantities, context)
    }

    #[test]
    fn derives_value_from_quantity_chain_state_and_haircut() {
        let (proof, quantities, context) = fixture();
        let verified = verify_evm_chainlink_valuation_v1(&proof, &quantities, &context).unwrap();
        assert_eq!(verified.gross_assets, 18_000_000_000);
    }

    #[test]
    fn rejects_quantity_price_policy_staleness_and_liability_substitution() {
        let (proof, quantities, context) = fixture();

        let mut wrong_quantity = quantities.clone();
        wrong_quantity[0].raw_quantity += U256::from(1u8);
        assert_eq!(
            verify_evm_chainlink_valuation_v1(&proof, &wrong_quantity, &context),
            Err(EvmChainlinkValuationError::QuantityMismatch)
        );

        let mut wrong_price = proof.clone();
        wrong_price.feeds[0].transmission.value += U256::from(1u8);
        assert_eq!(
            verify_evm_chainlink_valuation_v1(&wrong_price, &quantities, &context),
            Err(EvmChainlinkValuationError::OracleProof)
        );

        let mut wrong_policy = proof.clone();
        wrong_policy.policy.rows[0].haircut_bps = 10_000;
        assert_eq!(
            verify_evm_chainlink_valuation_v1(&wrong_policy, &quantities, &context),
            Err(EvmChainlinkValuationError::PolicyMismatch)
        );

        let mut stale = proof.clone();
        stale.checkpoint_certificate.checkpoint.source_timestamp_ms += 61_000;
        assert_eq!(
            verify_evm_chainlink_valuation_v1(&stale, &quantities, &context),
            Err(EvmChainlinkValuationError::CheckpointMismatch)
        );

        let mut liability_context = context;
        liability_context.expected_total_liabilities = 1;
        assert_eq!(
            verify_evm_chainlink_valuation_v1(&proof, &quantities, &liability_context),
            Err(EvmChainlinkValuationError::PolicyMismatch)
        );
    }
}
