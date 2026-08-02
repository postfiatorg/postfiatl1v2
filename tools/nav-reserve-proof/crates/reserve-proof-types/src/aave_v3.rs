//! Aave V3 collateral, debt, reserve-index, and oracle verification beneath
//! a governed quorum-certified EVM state root.

use alloy_primitives::{eip191_hash_message, keccak256, Address, Bytes, Signature, B256, U256};
use alloy_rlp::{encode, encode_fixed_size};
use alloy_trie::{nybbles::Nibbles, proof::verify_proof, TrieAccount};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};

use crate::bft_checkpoint::{BftSourceCheckpointCertificateV1, BftSourceCheckpointV1};
use crate::evm_checkpoint::{EvmAccountProofV1, EvmStorageProofV1};

pub const AAVE_V3_ADAPTER_KIND_V1: &str = "aave-v3-evm-state-proof-v1";
pub const AAVE_EVM_CHECKPOINT_KIND_V1: &str = "evm-state-root-v1";
pub const MAX_AAVE_POSITIONS: usize = 32;
pub const MAX_AAVE_PROOF_NODES: usize = 64;
pub const MAX_AAVE_PROOF_NODE_BYTES: usize = 64 * 1024;
pub const MAX_AAVE_PROOF_TOTAL_BYTES: usize = 512 * 1024;
pub const AAVE_RAY: u128 = 1_000_000_000_000_000_000_000_000_000;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_aave_v3_policy.v1";
const OWNER_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_aave_v3_owner.v1";
const OWNER_AUTHORIZATION_DOMAIN: &[u8] = b"postfiat.reserve_aave_v3_owner_authorization.v1";
const EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_aave_v3_evidence.v1";
const METADATA_DOMAIN: &[u8] = b"postfiat.reserve_aave_v3_metadata.v1";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AaveV3PositionKindV1 {
    Collateral,
    VariableDebt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AaveOracleSourcePolicyV1 {
    DirectChainlink {
        proxy_address: Address,
    },
    CappedStable {
        adapter_address: Address,
        adapter_code_hash: B256,
        chainlink_proxy_address: Address,
        price_cap_slot_index: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AaveV3PositionPolicyV1 {
    pub position_id: String,
    pub kind: AaveV3PositionKindV1,
    pub underlying_asset: Address,
    pub token_address: Address,
    pub token_code_hash: B256,
    /// Storage mapping slot for this token's per-user scaled balance.
    pub user_state_slot_index: U256,
    pub decimals: u8,
    pub chainlink_proxy_code_hash: B256,
    pub chainlink_aggregator_code_hash: B256,
    pub oracle_source: AaveOracleSourcePolicyV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AaveV3PolicyV1 {
    pub source_domain: String,
    pub ethereum_chain_id: u64,
    pub pool_address: Address,
    pub pool_code_hash: B256,
    pub oracle_address: Address,
    pub oracle_code_hash: B256,
    pub reserve_mapping_slot_index: u64,
    pub oracle_sources_slot_index: u64,
    pub chainlink_proxy_phase_slot_index: u64,
    pub chainlink_hot_vars_slot_index: u64,
    pub chainlink_transmissions_slot_index: u64,
    pub seconds_per_year: u64,
    pub max_oracle_age_seconds: u64,
    pub positions: Vec<AaveV3PositionPolicyV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AaveV3ReserveProofV1 {
    pub pool_account: EvmAccountProofV1,
    pub indexes_and_rates_1: EvmStorageProofV1,
    pub indexes_and_rates_2: EvmStorageProofV1,
    pub metadata: EvmStorageProofV1,
    pub a_token_address: EvmStorageProofV1,
    pub variable_debt_token_address: EvmStorageProofV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AaveOracleSourceProofV1 {
    DirectChainlink,
    CappedStable {
        adapter_account: Box<EvmAccountProofV1>,
        price_cap: Box<EvmStorageProofV1>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChainlinkFeedProofV1 {
    pub proxy_account: EvmAccountProofV1,
    pub current_phase: EvmStorageProofV1,
    pub aggregator_account: EvmAccountProofV1,
    pub hot_vars: EvmStorageProofV1,
    pub transmission: EvmStorageProofV1,
    pub decimals: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AaveOraclePriceProofV1 {
    pub oracle_account: EvmAccountProofV1,
    pub source: EvmStorageProofV1,
    pub source_kind: AaveOracleSourceProofV1,
    pub chainlink: ChainlinkFeedProofV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AaveV3PositionProofV1 {
    pub position_id: String,
    pub token_account: EvmAccountProofV1,
    pub user_state_slot_index: U256,
    pub user_state: EvmStorageProofV1,
    pub reserve: AaveV3ReserveProofV1,
    pub oracle: AaveOraclePriceProofV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AaveV3ProofV1 {
    pub policy: AaveV3PolicyV1,
    pub checkpoint_certificate: BftSourceCheckpointCertificateV1,
    pub owner: Address,
    pub ownership_signature: Vec<u8>,
    pub positions: Vec<AaveV3PositionProofV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AaveV3VerificationV1 {
    pub block_number: u64,
    pub block_hash: B256,
    pub state_root: B256,
    pub block_timestamp_seconds: u64,
    pub collateral_usd_e8: u64,
    pub liability_usd_e8: u64,
    pub evidence_commitment: String,
    pub metadata_hash: B256,
}

pub struct AaveV3VerifyContextV1<'a> {
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
    pub valuation_verifier_commitment: &'a str,
    pub observed_at_pftl_height: u64,
    pub expected_gross_assets: u64,
    pub expected_total_liabilities: u64,
    pub expected_evidence_commitment: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AaveV3Error {
    BoundsExceeded,
    PolicyMismatch,
    CheckpointMismatch,
    OwnerAuthorization,
    EvidenceCommitment,
    PositionMismatch,
    AccountProof,
    StorageProof,
    ReserveProof,
    OracleProof,
    OracleStale,
    BadTimestamp,
    BadPrice,
    ArithmeticOverflow,
}

impl AaveV3PolicyV1 {
    pub fn validate(&self) -> Result<(), AaveV3Error> {
        validate_identifier(&self.source_domain)?;
        if self.ethereum_chain_id == 0
            || self.source_domain != format!("eip155:{}", self.ethereum_chain_id)
            || self.pool_address == Address::ZERO
            || self.pool_code_hash == B256::ZERO
            || self.oracle_address == Address::ZERO
            || self.oracle_code_hash == B256::ZERO
            || self.reserve_mapping_slot_index == 0
            || self.chainlink_proxy_phase_slot_index == 0
            || self.chainlink_hot_vars_slot_index == 0
            || self.chainlink_transmissions_slot_index == 0
            || self.seconds_per_year == 0
            || self.max_oracle_age_seconds == 0
            || self.positions.is_empty()
            || self.positions.len() > MAX_AAVE_POSITIONS
        {
            return Err(AaveV3Error::PolicyMismatch);
        }
        let mut previous = None;
        for position in &self.positions {
            validate_identifier(&position.position_id)?;
            if previous >= Some(position.position_id.as_str())
                || position.underlying_asset == Address::ZERO
                || position.token_address == Address::ZERO
                || position.token_code_hash == B256::ZERO
                || position.chainlink_proxy_code_hash == B256::ZERO
                || position.chainlink_aggregator_code_hash == B256::ZERO
                || position.decimals > 38
            {
                return Err(AaveV3Error::PolicyMismatch);
            }
            previous = Some(position.position_id.as_str());
            match &position.oracle_source {
                AaveOracleSourcePolicyV1::DirectChainlink { proxy_address }
                    if *proxy_address == Address::ZERO =>
                {
                    return Err(AaveV3Error::PolicyMismatch)
                }
                AaveOracleSourcePolicyV1::CappedStable {
                    adapter_address,
                    adapter_code_hash,
                    chainlink_proxy_address,
                    ..
                } if *adapter_address == Address::ZERO
                    || *adapter_code_hash == B256::ZERO
                    || *chainlink_proxy_address == Address::ZERO =>
                {
                    return Err(AaveV3Error::PolicyMismatch)
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn commitment(&self, committee_root: &str) -> Result<String, AaveV3Error> {
        self.validate()?;
        validate_lower_hex(committee_root, 48)?;
        let mut out = Vec::new();
        append_bytes(&mut out, self.source_domain.as_bytes())?;
        out.extend_from_slice(&self.ethereum_chain_id.to_be_bytes());
        out.extend_from_slice(self.pool_address.as_slice());
        out.extend_from_slice(self.pool_code_hash.as_slice());
        out.extend_from_slice(self.oracle_address.as_slice());
        out.extend_from_slice(self.oracle_code_hash.as_slice());
        for value in [
            self.reserve_mapping_slot_index,
            self.oracle_sources_slot_index,
            self.chainlink_proxy_phase_slot_index,
            self.chainlink_hot_vars_slot_index,
            self.chainlink_transmissions_slot_index,
            self.seconds_per_year,
            self.max_oracle_age_seconds,
        ] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        append_u32(&mut out, self.positions.len())?;
        for position in &self.positions {
            append_bytes(&mut out, position.position_id.as_bytes())?;
            out.push(match position.kind {
                AaveV3PositionKindV1::Collateral => 1,
                AaveV3PositionKindV1::VariableDebt => 2,
            });
            out.extend_from_slice(position.underlying_asset.as_slice());
            out.extend_from_slice(position.token_address.as_slice());
            out.extend_from_slice(position.token_code_hash.as_slice());
            out.extend_from_slice(&position.user_state_slot_index.to_be_bytes::<32>());
            out.push(position.decimals);
            out.extend_from_slice(position.chainlink_proxy_code_hash.as_slice());
            out.extend_from_slice(position.chainlink_aggregator_code_hash.as_slice());
            match &position.oracle_source {
                AaveOracleSourcePolicyV1::DirectChainlink { proxy_address } => {
                    out.push(1);
                    out.extend_from_slice(proxy_address.as_slice());
                }
                AaveOracleSourcePolicyV1::CappedStable {
                    adapter_address,
                    adapter_code_hash,
                    chainlink_proxy_address,
                    price_cap_slot_index,
                } => {
                    out.push(2);
                    out.extend_from_slice(adapter_address.as_slice());
                    out.extend_from_slice(adapter_code_hash.as_slice());
                    out.extend_from_slice(chainlink_proxy_address.as_slice());
                    out.extend_from_slice(&price_cap_slot_index.to_be_bytes());
                }
            }
        }
        append_hex(&mut out, committee_root, 48)?;
        Ok(hash48(POLICY_COMMITMENT_DOMAIN, &[&out]))
    }
}

pub fn aave_v3_owner_commitment(owner: Address) -> String {
    hash48(OWNER_COMMITMENT_DOMAIN, &[owner.as_slice()])
}

pub fn verify_aave_v3_proof_v1(
    proof: &AaveV3ProofV1,
    context: &AaveV3VerifyContextV1<'_>,
) -> Result<AaveV3VerificationV1, AaveV3Error> {
    validate_proof_bounds(proof)?;
    validate_context(context)?;
    proof
        .checkpoint_certificate
        .verify()
        .map_err(|_| AaveV3Error::CheckpointMismatch)?;
    proof.policy.validate()?;
    let checkpoint = &proof.checkpoint_certificate.checkpoint;
    if checkpoint.pftl_genesis_hash != context.pftl_genesis_hash
        || checkpoint.checkpoint_kind != AAVE_EVM_CHECKPOINT_KIND_V1
        || checkpoint.source_domain != context.source_domain
        || checkpoint.source_domain != proof.policy.source_domain
        || checkpoint.pftl_observation_height != context.observed_at_pftl_height
    {
        return Err(AaveV3Error::CheckpointMismatch);
    }
    if checkpoint.source_timestamp_ms % 1_000 != 0 {
        return Err(AaveV3Error::BadTimestamp);
    }
    let block_timestamp_seconds = checkpoint.source_timestamp_ms / 1_000;
    let committee_root = proof
        .checkpoint_certificate
        .committee
        .root()
        .map_err(|_| AaveV3Error::CheckpointMismatch)?;
    let policy_commitment = proof.policy.commitment(&committee_root)?;
    if policy_commitment != context.quantity_verifier_commitment
        || policy_commitment != context.valuation_verifier_commitment
        || aave_v3_owner_commitment(proof.owner) != context.reserve_owner_commitment
        || context.asset_or_position_id
            != format!("aave-v3:account:0x{}", hex::encode(proof.owner.as_slice()))
    {
        return Err(AaveV3Error::PolicyMismatch);
    }
    let evidence_commitment = proof.commitment()?;
    if evidence_commitment != context.expected_evidence_commitment {
        return Err(AaveV3Error::EvidenceCommitment);
    }
    verify_owner_authorization(proof, context, &policy_commitment, checkpoint)?;

    if proof.positions.len() != proof.policy.positions.len() {
        return Err(AaveV3Error::PositionMismatch);
    }
    let mut collateral = 0u128;
    let mut liabilities = 0u128;
    for (position_proof, position_policy) in proof.positions.iter().zip(&proof.policy.positions) {
        if position_proof.position_id != position_policy.position_id {
            return Err(AaveV3Error::PositionMismatch);
        }
        let value = verify_position(
            &proof.policy,
            proof.owner,
            position_proof,
            position_policy,
            checkpoint.source_state_commitment,
            block_timestamp_seconds,
        )?;
        match position_policy.kind {
            AaveV3PositionKindV1::Collateral => {
                collateral = collateral
                    .checked_add(value)
                    .ok_or(AaveV3Error::ArithmeticOverflow)?;
            }
            AaveV3PositionKindV1::VariableDebt => {
                liabilities = liabilities
                    .checked_add(value)
                    .ok_or(AaveV3Error::ArithmeticOverflow)?;
            }
        }
    }
    let collateral_usd_e8 =
        u64::try_from(collateral).map_err(|_| AaveV3Error::ArithmeticOverflow)?;
    let liability_usd_e8 =
        u64::try_from(liabilities).map_err(|_| AaveV3Error::ArithmeticOverflow)?;
    if collateral_usd_e8 != context.expected_gross_assets
        || liability_usd_e8 != context.expected_total_liabilities
    {
        return Err(AaveV3Error::EvidenceCommitment);
    }
    let mut metadata = Vec::new();
    append_hex(&mut metadata, &policy_commitment, 48)?;
    metadata.extend_from_slice(checkpoint.source_block_hash.as_slice());
    metadata.extend_from_slice(checkpoint.source_state_commitment.as_slice());
    metadata.extend_from_slice(&collateral_usd_e8.to_be_bytes());
    metadata.extend_from_slice(&liability_usd_e8.to_be_bytes());

    Ok(AaveV3VerificationV1 {
        block_number: checkpoint.source_height,
        block_hash: checkpoint.source_block_hash,
        state_root: checkpoint.source_state_commitment,
        block_timestamp_seconds,
        collateral_usd_e8,
        liability_usd_e8,
        evidence_commitment,
        metadata_hash: keccak256(domain_message(METADATA_DOMAIN, &metadata)),
    })
}

impl AaveV3ProofV1 {
    pub fn commitment(&self) -> Result<String, AaveV3Error> {
        validate_proof_bounds(self)?;
        let committee_root = self
            .checkpoint_certificate
            .committee
            .root()
            .map_err(|_| AaveV3Error::CheckpointMismatch)?;
        let policy_commitment = self.policy.commitment(&committee_root)?;
        let mut out = Vec::new();
        append_hex(&mut out, &policy_commitment, 48)?;
        append_bytes(
            &mut out,
            &self
                .checkpoint_certificate
                .checkpoint
                .canonical_bytes()
                .map_err(|_| AaveV3Error::CheckpointMismatch)?,
        )?;
        append_u32(&mut out, self.checkpoint_certificate.votes.len())?;
        for vote in &self.checkpoint_certificate.votes {
            append_bytes(&mut out, vote.validator_id.as_bytes())?;
            append_bytes(&mut out, &vote.signature)?;
        }
        out.extend_from_slice(self.owner.as_slice());
        append_bytes(&mut out, &self.ownership_signature)?;
        append_u32(&mut out, self.positions.len())?;
        for position in &self.positions {
            append_position_proof(&mut out, position)?;
        }
        Ok(hash48(EVIDENCE_COMMITMENT_DOMAIN, &[&out]))
    }
}

pub fn aave_v3_owner_authorization_statement_v1(
    proof: &AaveV3ProofV1,
    context: &AaveV3VerifyContextV1<'_>,
) -> Result<Vec<u8>, AaveV3Error> {
    validate_proof_bounds(proof)?;
    validate_context(context)?;
    aave_v3_owner_authorization_statement_for_policy_v1(
        &proof.policy,
        &proof.checkpoint_certificate,
        proof.owner,
        context,
    )
}

pub fn aave_v3_owner_authorization_statement_for_policy_v1(
    policy: &AaveV3PolicyV1,
    checkpoint_certificate: &BftSourceCheckpointCertificateV1,
    owner: Address,
    context: &AaveV3VerifyContextV1<'_>,
) -> Result<Vec<u8>, AaveV3Error> {
    policy.validate()?;
    validate_context(context)?;
    checkpoint_certificate
        .verify()
        .map_err(|_| AaveV3Error::CheckpointMismatch)?;
    let committee_root = checkpoint_certificate
        .committee
        .root()
        .map_err(|_| AaveV3Error::CheckpointMismatch)?;
    let policy_commitment = policy.commitment(&committee_root)?;
    owner_authorization_statement(
        policy,
        owner,
        context,
        &policy_commitment,
        &checkpoint_certificate.checkpoint,
    )
}

fn verify_owner_authorization(
    proof: &AaveV3ProofV1,
    context: &AaveV3VerifyContextV1<'_>,
    policy_commitment: &str,
    checkpoint: &BftSourceCheckpointV1,
) -> Result<(), AaveV3Error> {
    let signature: [u8; 65] = proof
        .ownership_signature
        .as_slice()
        .try_into()
        .map_err(|_| AaveV3Error::OwnerAuthorization)?;
    let signature =
        Signature::from_raw_array(&signature).map_err(|_| AaveV3Error::OwnerAuthorization)?;
    let statement = owner_authorization_statement(
        &proof.policy,
        proof.owner,
        context,
        policy_commitment,
        checkpoint,
    )?;
    let recovered = signature
        .recover_address_from_prehash(&eip191_hash_message(&statement))
        .map_err(|_| AaveV3Error::OwnerAuthorization)?;
    if recovered == proof.owner {
        Ok(())
    } else {
        Err(AaveV3Error::OwnerAuthorization)
    }
}

fn owner_authorization_statement(
    policy: &AaveV3PolicyV1,
    owner: Address,
    context: &AaveV3VerifyContextV1<'_>,
    policy_commitment: &str,
    checkpoint: &BftSourceCheckpointV1,
) -> Result<Vec<u8>, AaveV3Error> {
    let mut out = Vec::new();
    append_hex(&mut out, context.pftl_genesis_hash, 48)?;
    append_hex(&mut out, context.nav_asset_id, 48)?;
    append_hex(&mut out, context.proof_profile_id, 48)?;
    append_hex(&mut out, context.valuation_policy_hash, 32)?;
    append_hex(&mut out, context.source_manifest_hash, 48)?;
    append_bytes(&mut out, context.source_id.as_bytes())?;
    append_bytes(&mut out, context.source_domain.as_bytes())?;
    append_bytes(&mut out, context.asset_or_position_id.as_bytes())?;
    append_hex(&mut out, context.reserve_owner_commitment, 48)?;
    append_hex(&mut out, policy_commitment, 48)?;
    out.extend_from_slice(&context.observed_at_pftl_height.to_be_bytes());
    append_bytes(
        &mut out,
        &checkpoint
            .canonical_bytes()
            .map_err(|_| AaveV3Error::CheckpointMismatch)?,
    )?;
    if policy.source_domain != checkpoint.source_domain {
        return Err(AaveV3Error::CheckpointMismatch);
    }
    out.extend_from_slice(owner.as_slice());
    Ok(domain_message(OWNER_AUTHORIZATION_DOMAIN, &out))
}

fn verify_position(
    overall_policy: &AaveV3PolicyV1,
    owner: Address,
    position: &AaveV3PositionProofV1,
    policy: &AaveV3PositionPolicyV1,
    state_root: B256,
    block_timestamp: u64,
) -> Result<u128, AaveV3Error> {
    if position.token_account.address != policy.token_address
        || position.token_account.code_hash != policy.token_code_hash
    {
        return Err(AaveV3Error::PositionMismatch);
    }
    verify_account_proof(state_root, &position.token_account)?;
    let expected_user_slot = mapping_slot_address(owner, policy.user_state_slot_index);
    if position.user_state_slot_index != policy.user_state_slot_index
        || position.user_state.key != expected_user_slot
    {
        return Err(AaveV3Error::PositionMismatch);
    }
    verify_storage_proof(position.token_account.storage_root, &position.user_state)?;
    let scaled = low_u128(position.user_state.value);
    let normalized_index = verify_reserve(
        overall_policy,
        policy,
        &position.reserve,
        state_root,
        block_timestamp,
    )?;
    let price = verify_oracle_price(
        overall_policy,
        policy,
        &position.oracle,
        state_root,
        block_timestamp,
    )?;
    let units = match policy.kind {
        AaveV3PositionKindV1::VariableDebt => ray_mul_ceil(scaled, normalized_index)?,
        AaveV3PositionKindV1::Collateral => ray_mul_floor(scaled, normalized_index)?,
    };
    match policy.kind {
        AaveV3PositionKindV1::VariableDebt => usd_e8_ceil(units, price, policy.decimals),
        AaveV3PositionKindV1::Collateral => usd_e8_floor(units, price, policy.decimals),
    }
}

fn verify_reserve(
    policy: &AaveV3PolicyV1,
    position: &AaveV3PositionPolicyV1,
    reserve: &AaveV3ReserveProofV1,
    state_root: B256,
    block_timestamp: u64,
) -> Result<u128, AaveV3Error> {
    if reserve.pool_account.address != policy.pool_address
        || reserve.pool_account.code_hash != policy.pool_code_hash
    {
        return Err(AaveV3Error::ReserveProof);
    }
    verify_account_proof(state_root, &reserve.pool_account)?;
    let slot_index = U256::from(policy.reserve_mapping_slot_index);
    for (slot, offset) in [
        (&reserve.indexes_and_rates_1, 1u64),
        (&reserve.indexes_and_rates_2, 2),
        (&reserve.metadata, 3),
        (&reserve.a_token_address, 4),
        (&reserve.variable_debt_token_address, 6),
    ] {
        if slot.key != aave_reserve_storage_slot(position.underlying_asset, slot_index, offset) {
            return Err(AaveV3Error::ReserveProof);
        }
        verify_storage_proof(reserve.pool_account.storage_root, slot)?;
    }
    let a_token = address_from_plain_storage(reserve.a_token_address.value)?;
    let debt_token = address_from_plain_storage(reserve.variable_debt_token_address.value)?;
    match position.kind {
        AaveV3PositionKindV1::Collateral if a_token != position.token_address => {
            return Err(AaveV3Error::ReserveProof)
        }
        AaveV3PositionKindV1::VariableDebt if debt_token != position.token_address => {
            return Err(AaveV3Error::ReserveProof)
        }
        _ => {}
    }
    let (index, rate) = match position.kind {
        AaveV3PositionKindV1::Collateral => (
            low_u128(reserve.indexes_and_rates_1.value),
            high_u128(reserve.indexes_and_rates_1.value),
        ),
        AaveV3PositionKindV1::VariableDebt => (
            low_u128(reserve.indexes_and_rates_2.value),
            high_u128(reserve.indexes_and_rates_2.value),
        ),
    };
    let last_update = extract_bits_u64(reserve.metadata.value, 128, 40)?;
    if block_timestamp < last_update {
        return Err(AaveV3Error::BadTimestamp);
    }
    if block_timestamp == last_update {
        return Ok(index);
    }
    let accrued = match position.kind {
        AaveV3PositionKindV1::Collateral => {
            calculate_linear_interest(rate, last_update, block_timestamp, policy.seconds_per_year)?
        }
        AaveV3PositionKindV1::VariableDebt => calculate_compounded_interest(
            rate,
            last_update,
            block_timestamp,
            policy.seconds_per_year,
        )?,
    };
    ray_mul_half_up(accrued, index)
}

fn verify_oracle_price(
    policy: &AaveV3PolicyV1,
    position: &AaveV3PositionPolicyV1,
    oracle: &AaveOraclePriceProofV1,
    state_root: B256,
    block_timestamp: u64,
) -> Result<u128, AaveV3Error> {
    if oracle.oracle_account.address != policy.oracle_address
        || oracle.oracle_account.code_hash != policy.oracle_code_hash
    {
        return Err(AaveV3Error::OracleProof);
    }
    verify_account_proof(state_root, &oracle.oracle_account)?;
    if oracle.source.key
        != mapping_slot_address(
            position.underlying_asset,
            U256::from(policy.oracle_sources_slot_index),
        )
    {
        return Err(AaveV3Error::OracleProof);
    }
    verify_storage_proof(oracle.oracle_account.storage_root, &oracle.source)?;
    let source = address_from_plain_storage(oracle.source.value)?;
    let chainlink_price = verify_chainlink_price(
        policy,
        position,
        &oracle.chainlink,
        state_root,
        block_timestamp,
    )?;
    match (&position.oracle_source, &oracle.source_kind) {
        (
            AaveOracleSourcePolicyV1::DirectChainlink { proxy_address },
            AaveOracleSourceProofV1::DirectChainlink,
        ) if source == *proxy_address
            && oracle.chainlink.proxy_account.address == *proxy_address =>
        {
            Ok(chainlink_price)
        }
        (
            AaveOracleSourcePolicyV1::CappedStable {
                adapter_address,
                adapter_code_hash,
                chainlink_proxy_address,
                price_cap_slot_index,
            },
            AaveOracleSourceProofV1::CappedStable {
                adapter_account,
                price_cap,
            },
        ) if source == *adapter_address
            && adapter_account.address == *adapter_address
            && adapter_account.code_hash == *adapter_code_hash
            && oracle.chainlink.proxy_account.address == *chainlink_proxy_address =>
        {
            verify_account_proof(state_root, adapter_account)?;
            if price_cap.key != fixed_storage_slot(*price_cap_slot_index) {
                return Err(AaveV3Error::OracleProof);
            }
            verify_storage_proof(adapter_account.storage_root, price_cap)?;
            let cap = u256_to_u128(price_cap.value)?;
            Ok(chainlink_price.min(cap))
        }
        _ => Err(AaveV3Error::OracleProof),
    }
}

fn verify_chainlink_price(
    policy: &AaveV3PolicyV1,
    position: &AaveV3PositionPolicyV1,
    feed: &ChainlinkFeedProofV1,
    state_root: B256,
    block_timestamp: u64,
) -> Result<u128, AaveV3Error> {
    if feed.decimals != 8 {
        return Err(AaveV3Error::OracleProof);
    }
    if feed.proxy_account.code_hash != position.chainlink_proxy_code_hash
        || feed.aggregator_account.code_hash != position.chainlink_aggregator_code_hash
    {
        return Err(AaveV3Error::OracleProof);
    }
    verify_account_proof(state_root, &feed.proxy_account)?;
    if feed.current_phase.key != fixed_storage_slot(policy.chainlink_proxy_phase_slot_index) {
        return Err(AaveV3Error::OracleProof);
    }
    verify_storage_proof(feed.proxy_account.storage_root, &feed.current_phase)?;
    let aggregator = current_phase_aggregator(feed.current_phase.value)?;
    if aggregator != feed.aggregator_account.address {
        return Err(AaveV3Error::OracleProof);
    }
    verify_account_proof(state_root, &feed.aggregator_account)?;
    if feed.hot_vars.key != fixed_storage_slot(policy.chainlink_hot_vars_slot_index) {
        return Err(AaveV3Error::OracleProof);
    }
    verify_storage_proof(feed.aggregator_account.storage_root, &feed.hot_vars)?;
    let latest_round = chainlink_latest_round(feed.hot_vars.value)?;
    if feed.transmission.key
        != chainlink_transmission_slot(latest_round, policy.chainlink_transmissions_slot_index)
    {
        return Err(AaveV3Error::OracleProof);
    }
    verify_storage_proof(feed.aggregator_account.storage_root, &feed.transmission)?;
    let (answer, observation_timestamp, transmission_timestamp) =
        transmission_answer_and_timestamps(feed.transmission.value)?;
    if transmission_timestamp < observation_timestamp
        || transmission_timestamp > block_timestamp
        || block_timestamp
            .checked_sub(transmission_timestamp)
            .ok_or(AaveV3Error::BadTimestamp)?
            > policy.max_oracle_age_seconds
    {
        return Err(AaveV3Error::OracleStale);
    }
    Ok(answer)
}

fn validate_proof_bounds(proof: &AaveV3ProofV1) -> Result<(), AaveV3Error> {
    proof.policy.validate()?;
    if proof.ownership_signature.len() != 65
        || proof.positions.len() != proof.policy.positions.len()
        || proof.positions.len() > MAX_AAVE_POSITIONS
    {
        return Err(AaveV3Error::BoundsExceeded);
    }
    for position in &proof.positions {
        validate_identifier(&position.position_id)?;
        validate_account_proof(&position.token_account)?;
        validate_storage_proof(&position.user_state)?;
        validate_account_proof(&position.reserve.pool_account)?;
        for storage in [
            &position.reserve.indexes_and_rates_1,
            &position.reserve.indexes_and_rates_2,
            &position.reserve.metadata,
            &position.reserve.a_token_address,
            &position.reserve.variable_debt_token_address,
        ] {
            validate_storage_proof(storage)?;
        }
        validate_account_proof(&position.oracle.oracle_account)?;
        validate_storage_proof(&position.oracle.source)?;
        validate_account_proof(&position.oracle.chainlink.proxy_account)?;
        validate_storage_proof(&position.oracle.chainlink.current_phase)?;
        validate_account_proof(&position.oracle.chainlink.aggregator_account)?;
        validate_storage_proof(&position.oracle.chainlink.hot_vars)?;
        validate_storage_proof(&position.oracle.chainlink.transmission)?;
        if let AaveOracleSourceProofV1::CappedStable {
            adapter_account,
            price_cap,
        } = &position.oracle.source_kind
        {
            validate_account_proof(adapter_account)?;
            validate_storage_proof(price_cap)?;
        }
    }
    Ok(())
}

fn validate_context(context: &AaveV3VerifyContextV1<'_>) -> Result<(), AaveV3Error> {
    validate_lower_hex(context.pftl_genesis_hash, 48)?;
    validate_lower_hex(context.nav_asset_id, 48)?;
    validate_lower_hex(context.proof_profile_id, 48)?;
    validate_lower_hex(context.valuation_policy_hash, 32)?;
    validate_lower_hex(context.source_manifest_hash, 48)?;
    validate_lower_hex(context.reserve_owner_commitment, 48)?;
    validate_lower_hex(context.quantity_verifier_commitment, 48)?;
    validate_lower_hex(context.valuation_verifier_commitment, 48)?;
    validate_lower_hex(context.expected_evidence_commitment, 48)?;
    validate_identifier(context.source_id)?;
    validate_identifier(context.source_domain)?;
    validate_text(context.asset_or_position_id, 256)?;
    if context.observed_at_pftl_height == 0 {
        return Err(AaveV3Error::PolicyMismatch);
    }
    Ok(())
}

fn verify_account_proof(state_root: B256, account: &EvmAccountProofV1) -> Result<(), AaveV3Error> {
    validate_account_proof(account)?;
    let nodes = account
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
        state_root,
        Nibbles::unpack(keccak256(account.address.as_slice())),
        Some(expected),
        nodes.iter(),
    )
    .map_err(|_| AaveV3Error::AccountProof)
}

fn verify_storage_proof(
    storage_root: B256,
    storage: &EvmStorageProofV1,
) -> Result<(), AaveV3Error> {
    validate_storage_proof(storage)?;
    let nodes = storage
        .proof
        .iter()
        .cloned()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    let expected = if storage.value == U256::ZERO {
        None
    } else {
        Some(encode_fixed_size(&storage.value).as_ref().to_vec())
    };
    verify_proof(
        storage_root,
        Nibbles::unpack(keccak256(storage.key.as_slice())),
        expected,
        nodes.iter(),
    )
    .map_err(|_| AaveV3Error::StorageProof)
}

fn validate_account_proof(account: &EvmAccountProofV1) -> Result<(), AaveV3Error> {
    if account.address == Address::ZERO
        || account.storage_root == B256::ZERO
        || account.code_hash == B256::ZERO
    {
        return Err(AaveV3Error::AccountProof);
    }
    validate_nodes(&account.proof)
}

fn validate_storage_proof(storage: &EvmStorageProofV1) -> Result<(), AaveV3Error> {
    validate_nodes(&storage.proof)
}

fn validate_nodes(nodes: &[Vec<u8>]) -> Result<(), AaveV3Error> {
    if nodes.is_empty() || nodes.len() > MAX_AAVE_PROOF_NODES {
        return Err(AaveV3Error::BoundsExceeded);
    }
    let mut total = 0usize;
    for node in nodes {
        if node.is_empty() || node.len() > MAX_AAVE_PROOF_NODE_BYTES {
            return Err(AaveV3Error::BoundsExceeded);
        }
        total = total
            .checked_add(node.len())
            .ok_or(AaveV3Error::ArithmeticOverflow)?;
    }
    if total > MAX_AAVE_PROOF_TOTAL_BYTES {
        return Err(AaveV3Error::BoundsExceeded);
    }
    Ok(())
}

fn append_position_proof(
    out: &mut Vec<u8>,
    position: &AaveV3PositionProofV1,
) -> Result<(), AaveV3Error> {
    append_bytes(out, position.position_id.as_bytes())?;
    append_account_proof(out, &position.token_account)?;
    out.extend_from_slice(&position.user_state_slot_index.to_be_bytes::<32>());
    append_storage_proof(out, &position.user_state)?;
    append_account_proof(out, &position.reserve.pool_account)?;
    for storage in [
        &position.reserve.indexes_and_rates_1,
        &position.reserve.indexes_and_rates_2,
        &position.reserve.metadata,
        &position.reserve.a_token_address,
        &position.reserve.variable_debt_token_address,
    ] {
        append_storage_proof(out, storage)?;
    }
    append_account_proof(out, &position.oracle.oracle_account)?;
    append_storage_proof(out, &position.oracle.source)?;
    match &position.oracle.source_kind {
        AaveOracleSourceProofV1::DirectChainlink => out.push(1),
        AaveOracleSourceProofV1::CappedStable {
            adapter_account,
            price_cap,
        } => {
            out.push(2);
            append_account_proof(out, adapter_account)?;
            append_storage_proof(out, price_cap)?;
        }
    }
    append_account_proof(out, &position.oracle.chainlink.proxy_account)?;
    append_storage_proof(out, &position.oracle.chainlink.current_phase)?;
    append_account_proof(out, &position.oracle.chainlink.aggregator_account)?;
    append_storage_proof(out, &position.oracle.chainlink.hot_vars)?;
    append_storage_proof(out, &position.oracle.chainlink.transmission)?;
    out.push(position.oracle.chainlink.decimals);
    Ok(())
}

fn append_account_proof(out: &mut Vec<u8>, account: &EvmAccountProofV1) -> Result<(), AaveV3Error> {
    validate_account_proof(account)?;
    out.extend_from_slice(account.address.as_slice());
    out.extend_from_slice(&account.nonce.to_be_bytes());
    out.extend_from_slice(&account.balance.to_be_bytes::<32>());
    out.extend_from_slice(account.storage_root.as_slice());
    out.extend_from_slice(account.code_hash.as_slice());
    append_nodes(out, &account.proof)
}

fn append_storage_proof(out: &mut Vec<u8>, storage: &EvmStorageProofV1) -> Result<(), AaveV3Error> {
    validate_storage_proof(storage)?;
    out.extend_from_slice(storage.key.as_slice());
    out.extend_from_slice(&storage.value.to_be_bytes::<32>());
    append_nodes(out, &storage.proof)
}

fn append_nodes(out: &mut Vec<u8>, nodes: &[Vec<u8>]) -> Result<(), AaveV3Error> {
    validate_nodes(nodes)?;
    append_u32(out, nodes.len())?;
    for node in nodes {
        append_bytes(out, node)?;
    }
    Ok(())
}

pub fn mapping_slot_address(address: Address, slot_index: U256) -> B256 {
    let mut encoded = [0u8; 64];
    encoded[12..32].copy_from_slice(address.as_slice());
    encoded[32..].copy_from_slice(&slot_index.to_be_bytes::<32>());
    keccak256(encoded)
}

pub fn aave_reserve_storage_slot(asset: Address, slot_index: U256, offset: u64) -> B256 {
    let base = mapping_slot_address(asset, slot_index);
    B256::from((U256::from_be_bytes(base.0) + U256::from(offset)).to_be_bytes::<32>())
}

pub fn fixed_storage_slot(slot_index: u64) -> B256 {
    B256::from(U256::from(slot_index).to_be_bytes::<32>())
}

pub fn chainlink_transmission_slot(round_id: u32, mapping_slot_index: u64) -> B256 {
    let mut encoded = [0u8; 64];
    encoded[28..32].copy_from_slice(&round_id.to_be_bytes());
    encoded[32..].copy_from_slice(&U256::from(mapping_slot_index).to_be_bytes::<32>());
    keccak256(encoded)
}

fn address_from_plain_storage(value: U256) -> Result<Address, AaveV3Error> {
    if (value >> 160usize) != U256::ZERO {
        return Err(AaveV3Error::OracleProof);
    }
    Ok(address_from_low_160(value))
}

fn address_from_low_160(value: U256) -> Address {
    let bytes = value.to_be_bytes::<32>();
    Address::from_slice(&bytes[12..])
}

pub fn current_phase_aggregator(value: U256) -> Result<Address, AaveV3Error> {
    let phase_id = value & U256::from(u16::MAX);
    let aggregator =
        address_from_low_160((value >> 16usize) & ((U256::from(1) << 160usize) - U256::from(1)));
    if phase_id == U256::ZERO || aggregator == Address::ZERO {
        return Err(AaveV3Error::OracleProof);
    }
    Ok(aggregator)
}

pub fn chainlink_latest_round(value: U256) -> Result<u32, AaveV3Error> {
    let round = ((value >> 48usize) & U256::from(u32::MAX)).to::<u32>();
    if round == 0 {
        Err(AaveV3Error::OracleProof)
    } else {
        Ok(round)
    }
}

fn transmission_answer_and_timestamps(value: U256) -> Result<(u128, u64, u64), AaveV3Error> {
    let answer = value & ((U256::from(1) << 192usize) - U256::from(1));
    if answer == U256::ZERO || ((answer >> 191usize) & U256::from(1)) != U256::ZERO {
        return Err(AaveV3Error::BadPrice);
    }
    let observation_timestamp = ((value >> 192usize) & U256::from(u32::MAX)).to::<u64>();
    let transmission_timestamp = ((value >> 224usize) & U256::from(u32::MAX)).to::<u64>();
    if observation_timestamp == 0 || transmission_timestamp == 0 {
        return Err(AaveV3Error::OracleStale);
    }
    Ok((
        u256_to_u128(answer)?,
        observation_timestamp,
        transmission_timestamp,
    ))
}

fn low_u128(value: U256) -> u128 {
    (value & U256::from(u128::MAX)).to::<u128>()
}

fn high_u128(value: U256) -> u128 {
    ((value >> 128usize) & U256::from(u128::MAX)).to::<u128>()
}

fn extract_bits_u64(value: U256, shift: usize, bits: usize) -> Result<u64, AaveV3Error> {
    if bits == 0 || bits > 64 {
        return Err(AaveV3Error::ArithmeticOverflow);
    }
    let mask = (U256::from(1) << bits) - U256::from(1);
    Ok(((value >> shift) & mask).to::<u64>())
}

fn calculate_linear_interest(
    rate: u128,
    last_update: u64,
    current: u64,
    seconds_per_year: u64,
) -> Result<u128, AaveV3Error> {
    let delta = current
        .checked_sub(last_update)
        .ok_or(AaveV3Error::BadTimestamp)?;
    let accrued = U256::from(rate)
        .checked_mul(U256::from(delta))
        .ok_or(AaveV3Error::ArithmeticOverflow)?
        / U256::from(seconds_per_year);
    u256_to_u128(
        U256::from(AAVE_RAY)
            .checked_add(accrued)
            .ok_or(AaveV3Error::ArithmeticOverflow)?,
    )
}

fn calculate_compounded_interest(
    rate: u128,
    last_update: u64,
    current: u64,
    seconds_per_year: u64,
) -> Result<u128, AaveV3Error> {
    let exp = current
        .checked_sub(last_update)
        .ok_or(AaveV3Error::BadTimestamp)?;
    if exp == 0 {
        return Ok(AAVE_RAY);
    }
    let x = U256::from(rate)
        .checked_mul(U256::from(exp))
        .ok_or(AaveV3Error::ArithmeticOverflow)?
        / U256::from(seconds_per_year);
    let x_u128 = u256_to_u128(x)?;
    let x_over_6 = u256_to_u128(x / U256::from(6))?;
    let squared = ray_mul_half_up(x_u128, x_over_6)?;
    let inner = U256::from(x / U256::from(2))
        .checked_add(U256::from(squared))
        .ok_or(AaveV3Error::ArithmeticOverflow)?;
    let compounded = ray_mul_half_up(x_u128, u256_to_u128(inner)?)?;
    u256_to_u128(
        U256::from(AAVE_RAY)
            .checked_add(x)
            .and_then(|value| value.checked_add(U256::from(compounded)))
            .ok_or(AaveV3Error::ArithmeticOverflow)?,
    )
}

fn ray_mul_half_up(a: u128, b: u128) -> Result<u128, AaveV3Error> {
    u256_to_u128(
        U256::from(a)
            .checked_mul(U256::from(b))
            .and_then(|value| value.checked_add(U256::from(AAVE_RAY / 2)))
            .ok_or(AaveV3Error::ArithmeticOverflow)?
            / U256::from(AAVE_RAY),
    )
}

fn ray_mul_floor(a: u128, b: u128) -> Result<u128, AaveV3Error> {
    u256_to_u128(
        U256::from(a)
            .checked_mul(U256::from(b))
            .ok_or(AaveV3Error::ArithmeticOverflow)?
            / U256::from(AAVE_RAY),
    )
}

fn ray_mul_ceil(a: u128, b: u128) -> Result<u128, AaveV3Error> {
    mul_div_ceil(U256::from(a), U256::from(b), U256::from(AAVE_RAY))
}

fn usd_e8_floor(amount: u128, price: u128, decimals: u8) -> Result<u128, AaveV3Error> {
    let unit = checked_pow10(decimals)?;
    u256_to_u128(
        U256::from(amount)
            .checked_mul(U256::from(price))
            .ok_or(AaveV3Error::ArithmeticOverflow)?
            / U256::from(unit),
    )
}

fn usd_e8_ceil(amount: u128, price: u128, decimals: u8) -> Result<u128, AaveV3Error> {
    mul_div_ceil(
        U256::from(amount),
        U256::from(price),
        U256::from(checked_pow10(decimals)?),
    )
}

fn checked_pow10(decimals: u8) -> Result<u128, AaveV3Error> {
    10u128
        .checked_pow(u32::from(decimals))
        .ok_or(AaveV3Error::ArithmeticOverflow)
}

fn mul_div_ceil(a: U256, b: U256, divisor: U256) -> Result<u128, AaveV3Error> {
    if divisor == U256::ZERO {
        return Err(AaveV3Error::ArithmeticOverflow);
    }
    let product = a.checked_mul(b).ok_or(AaveV3Error::ArithmeticOverflow)?;
    let quotient = product / divisor;
    let remainder = product % divisor;
    u256_to_u128(if remainder == U256::ZERO {
        quotient
    } else {
        quotient
            .checked_add(U256::from(1))
            .ok_or(AaveV3Error::ArithmeticOverflow)?
    })
}

fn u256_to_u128(value: U256) -> Result<u128, AaveV3Error> {
    if value > U256::from(u128::MAX) {
        Err(AaveV3Error::ArithmeticOverflow)
    } else {
        Ok(value.to::<u128>())
    }
}

fn validate_identifier(value: &str) -> Result<(), AaveV3Error> {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(AaveV3Error::PolicyMismatch);
    }
    Ok(())
}

fn validate_text(value: &str, max: usize) -> Result<(), AaveV3Error> {
    if value.is_empty() || value.len() > max || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(AaveV3Error::PolicyMismatch);
    }
    Ok(())
}

fn validate_lower_hex(value: &str, bytes: usize) -> Result<(), AaveV3Error> {
    if value.len() != bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(AaveV3Error::PolicyMismatch);
    }
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), AaveV3Error> {
    validate_lower_hex(value, bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| AaveV3Error::PolicyMismatch)?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), AaveV3Error> {
    let len = u32::try_from(value.len()).map_err(|_| AaveV3Error::BoundsExceeded)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), AaveV3Error> {
    let value = u32::try_from(value).map_err(|_| AaveV3Error::BoundsExceeded)?;
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
        BftCheckpointCommitteeV1, BftCheckpointValidatorV1, BftSourceCheckpointCertificateV1,
        BftSourceCheckpointVoteV1, BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
    };
    use crate::{
        verify_observation_evidence, EvidenceDimensionV1, FreshnessPolicyV1, LiabilityTreatmentV1,
        ReserveProofContextV1, SourceEvidenceV1, SourceManifestEntryV1, SourceObservationV1,
        TrustClassV1,
    };
    use alloy_trie::{proof::ProofRetainer, HashBuilder};
    use k256::ecdsa::SigningKey;
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    const HISTORICAL_WITNESS: &str = include_str!(concat!(
        "../../../../../docs/evidence/a666-variable-size-nav-roundtrip-20260728/",
        "stake",
        "hub-nav-mark/proof/aave-witness.json"
    ));

    #[derive(Deserialize)]
    struct HistoricalWitness {
        owner: Address,
        block_timestamp: u64,
        state_root: B256,
        debt: HistoricalLeg,
        collateral: HistoricalLeg,
    }

    #[derive(Deserialize)]
    struct HistoricalLeg {
        underlying_asset: Address,
        token_account: EvmAccountProofV1,
        user_state_slot_index: U256,
        user_state: EvmStorageProofV1,
        reserve: HistoricalReserve,
        oracle: HistoricalOracle,
        decimals: u8,
    }

    #[derive(Deserialize)]
    struct HistoricalReserve {
        pool_account: EvmAccountProofV1,
        reserve_mapping_slot_index: U256,
        indexes_and_rates_1: EvmStorageProofV1,
        indexes_and_rates_2: EvmStorageProofV1,
        metadata: EvmStorageProofV1,
        a_token_address: EvmStorageProofV1,
        variable_debt_token_address: EvmStorageProofV1,
    }

    #[derive(Deserialize)]
    struct HistoricalOracle {
        aave_oracle_account: EvmAccountProofV1,
        source: EvmStorageProofV1,
        source_kind: HistoricalOracleSourceProof,
        chainlink: ChainlinkFeedProofV1,
    }

    #[derive(Deserialize)]
    enum HistoricalOracleSourceProof {
        DirectChainlink,
        CappedStable {
            adapter_account: Box<EvmAccountProofV1>,
            price_cap: Box<EvmStorageProofV1>,
        },
    }

    fn position_proof(position_id: &str, leg: HistoricalLeg) -> AaveV3PositionProofV1 {
        AaveV3PositionProofV1 {
            position_id: position_id.to_string(),
            token_account: leg.token_account,
            user_state_slot_index: leg.user_state_slot_index,
            user_state: leg.user_state,
            reserve: AaveV3ReserveProofV1 {
                pool_account: leg.reserve.pool_account,
                indexes_and_rates_1: leg.reserve.indexes_and_rates_1,
                indexes_and_rates_2: leg.reserve.indexes_and_rates_2,
                metadata: leg.reserve.metadata,
                a_token_address: leg.reserve.a_token_address,
                variable_debt_token_address: leg.reserve.variable_debt_token_address,
            },
            oracle: AaveOraclePriceProofV1 {
                oracle_account: leg.oracle.aave_oracle_account,
                source: leg.oracle.source,
                source_kind: match leg.oracle.source_kind {
                    HistoricalOracleSourceProof::DirectChainlink => {
                        AaveOracleSourceProofV1::DirectChainlink
                    }
                    HistoricalOracleSourceProof::CappedStable {
                        adapter_account,
                        price_cap,
                    } => AaveOracleSourceProofV1::CappedStable {
                        adapter_account,
                        price_cap,
                    },
                },
                chainlink: leg.oracle.chainlink,
            },
        }
    }

    fn trie_leaves(entries: &[(B256, Vec<u8>)]) -> Vec<(Nibbles, Vec<u8>)> {
        let mut leaves = entries
            .iter()
            .map(|(key, value)| (Nibbles::unpack(keccak256(key.as_slice())), value.clone()))
            .collect::<Vec<_>>();
        leaves.sort_by_key(|left| left.0);
        leaves
    }

    fn storage_root_and_proof(entries: &[(B256, U256)], target: B256) -> (B256, Vec<Vec<u8>>) {
        let encoded = entries
            .iter()
            .map(|(key, value)| (*key, encode_fixed_size(value).as_ref().to_vec()))
            .collect::<Vec<_>>();
        let target_path = Nibbles::unpack(keccak256(target.as_slice()));
        let mut builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([target_path]));
        for (path, value) in trie_leaves(&encoded) {
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
        let mut encoded = accounts
            .iter()
            .map(|(address, account)| {
                (
                    Nibbles::unpack(keccak256(address.as_slice())),
                    encode(*account),
                )
            })
            .collect::<Vec<_>>();
        encoded.sort_by_key(|left| left.0);
        let target_path = Nibbles::unpack(keccak256(target.as_slice()));
        let mut builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([target_path]));
        for (path, value) in encoded {
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

    fn storage_proof(entries: &[(B256, U256)], key: B256) -> EvmStorageProofV1 {
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
    }

    fn committee() -> (BftCheckpointCommitteeV1, Vec<MlDsa65KeyPair>) {
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        (
            BftCheckpointCommitteeV1 {
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
            },
            keys,
        )
    }

    fn synthetic_fixture() -> (AaveV3ProofV1, AaveV3VerifyContextV1<'static>) {
        let owner_key = SigningKey::from_bytes((&[0x21; 32]).into()).unwrap();
        let owner = Address::from_private_key(&owner_key);
        let underlying = Address::repeat_byte(0x31);
        let token = Address::repeat_byte(0x32);
        let debt_token = Address::repeat_byte(0x33);
        let pool = Address::repeat_byte(0x34);
        let oracle = Address::repeat_byte(0x35);
        let proxy = Address::repeat_byte(0x36);
        let aggregator = Address::repeat_byte(0x37);
        let timestamp = 1_785_000_000u64;
        let user_slot_index = U256::from(9);
        let user_key = mapping_slot_address(owner, user_slot_index);
        let token_storage = vec![(user_key, U256::from(1_000_000_000_000_000_000u128))];
        let (token_storage_root, _) = storage_root_and_proof(&token_storage, user_key);

        let reserve_slot = U256::from(52);
        let reserve_keys = [
            aave_reserve_storage_slot(underlying, reserve_slot, 1),
            aave_reserve_storage_slot(underlying, reserve_slot, 2),
            aave_reserve_storage_slot(underlying, reserve_slot, 3),
            aave_reserve_storage_slot(underlying, reserve_slot, 4),
            aave_reserve_storage_slot(underlying, reserve_slot, 6),
        ];
        let pool_storage = vec![
            (reserve_keys[0], U256::from(AAVE_RAY)),
            (reserve_keys[1], U256::from(AAVE_RAY)),
            (reserve_keys[2], U256::from(timestamp) << 128usize),
            (reserve_keys[3], U256::from_be_slice(token.as_slice())),
            (reserve_keys[4], U256::from_be_slice(debt_token.as_slice())),
        ];
        let (pool_storage_root, _) = storage_root_and_proof(&pool_storage, reserve_keys[0]);

        let oracle_key = mapping_slot_address(underlying, U256::ZERO);
        let oracle_storage = vec![(oracle_key, U256::from_be_slice(proxy.as_slice()))];
        let (oracle_storage_root, _) = storage_root_and_proof(&oracle_storage, oracle_key);

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
            | U256::from(200_000_000u64);
        let aggregator_storage = vec![(hot_key, hot_value), (transmission_key, transmission_value)];
        let (aggregator_storage_root, _) = storage_root_and_proof(&aggregator_storage, hot_key);

        let code_hash = B256::repeat_byte(0x99);
        let accounts = vec![
            (
                token,
                TrieAccount {
                    nonce: 1,
                    balance: U256::ZERO,
                    storage_root: token_storage_root,
                    code_hash,
                },
            ),
            (
                pool,
                TrieAccount {
                    nonce: 1,
                    balance: U256::ZERO,
                    storage_root: pool_storage_root,
                    code_hash,
                },
            ),
            (
                oracle,
                TrieAccount {
                    nonce: 1,
                    balance: U256::ZERO,
                    storage_root: oracle_storage_root,
                    code_hash,
                },
            ),
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
        let (state_root, _) = account_root_and_proof(&accounts, token);
        let account = |address: Address, storage_root: B256| {
            let (_, nodes) = account_root_and_proof(&accounts, address);
            EvmAccountProofV1 {
                address,
                nonce: 1,
                balance: U256::ZERO,
                storage_root,
                code_hash,
                proof: nodes,
            }
        };
        let position_policy = AaveV3PositionPolicyV1 {
            position_id: "collateral-test".to_string(),
            kind: AaveV3PositionKindV1::Collateral,
            underlying_asset: underlying,
            token_address: token,
            token_code_hash: code_hash,
            user_state_slot_index: user_slot_index,
            decimals: 18,
            chainlink_proxy_code_hash: code_hash,
            chainlink_aggregator_code_hash: code_hash,
            oracle_source: AaveOracleSourcePolicyV1::DirectChainlink {
                proxy_address: proxy,
            },
        };
        let policy = AaveV3PolicyV1 {
            source_domain: "eip155:42161".to_string(),
            ethereum_chain_id: 42_161,
            pool_address: pool,
            pool_code_hash: code_hash,
            oracle_address: oracle,
            oracle_code_hash: code_hash,
            reserve_mapping_slot_index: 52,
            oracle_sources_slot_index: 0,
            chainlink_proxy_phase_slot_index: 2,
            chainlink_hot_vars_slot_index: 13,
            chainlink_transmissions_slot_index: 17,
            seconds_per_year: 31_536_000,
            max_oracle_age_seconds: 60,
            positions: vec![position_policy],
        };
        let position = AaveV3PositionProofV1 {
            position_id: "collateral-test".to_string(),
            token_account: account(token, token_storage_root),
            user_state_slot_index: user_slot_index,
            user_state: storage_proof(&token_storage, user_key),
            reserve: AaveV3ReserveProofV1 {
                pool_account: account(pool, pool_storage_root),
                indexes_and_rates_1: storage_proof(&pool_storage, reserve_keys[0]),
                indexes_and_rates_2: storage_proof(&pool_storage, reserve_keys[1]),
                metadata: storage_proof(&pool_storage, reserve_keys[2]),
                a_token_address: storage_proof(&pool_storage, reserve_keys[3]),
                variable_debt_token_address: storage_proof(&pool_storage, reserve_keys[4]),
            },
            oracle: AaveOraclePriceProofV1 {
                oracle_account: account(oracle, oracle_storage_root),
                source: storage_proof(&oracle_storage, oracle_key),
                source_kind: AaveOracleSourceProofV1::DirectChainlink,
                chainlink: ChainlinkFeedProofV1 {
                    proxy_account: account(proxy, proxy_storage_root),
                    current_phase: storage_proof(&proxy_storage, phase_key),
                    aggregator_account: account(aggregator, aggregator_storage_root),
                    hot_vars: storage_proof(&aggregator_storage, hot_key),
                    transmission: storage_proof(&aggregator_storage, transmission_key),
                    decimals: 8,
                },
            },
        };
        let (committee, keys) = committee();
        let committee_root = committee.root().unwrap();
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: AAVE_EVM_CHECKPOINT_KIND_V1.to_string(),
            source_domain: "eip155:42161".to_string(),
            source_height: 100,
            source_timestamp_ms: timestamp * 1_000,
            source_block_hash: B256::repeat_byte(0x55),
            source_state_commitment: state_root,
            observed_source_head: 112,
            minimum_depth: 12,
            pftl_observation_height: 200,
            committee_epoch: committee.epoch,
            committee_root: committee_root.clone(),
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
                &[0xb0 + index as u8; 32],
            )
            .unwrap();
        }
        let policy_commitment = policy.commitment(&committee_root).unwrap();
        let mut proof = AaveV3ProofV1 {
            policy,
            checkpoint_certificate: certificate,
            owner,
            ownership_signature: vec![0; 65],
            positions: vec![position],
        };
        let owner_commitment = Box::leak(aave_v3_owner_commitment(owner).into_boxed_str());
        let policy_static = Box::leak(policy_commitment.into_boxed_str());
        let position_static = Box::leak(
            format!("aave-v3:account:0x{}", hex::encode(owner.as_slice())).into_boxed_str(),
        );
        let placeholder = AaveV3VerifyContextV1 {
            pftl_genesis_hash: Box::leak("11".repeat(48).into_boxed_str()),
            nav_asset_id: Box::leak("22".repeat(48).into_boxed_str()),
            proof_profile_id: Box::leak("33".repeat(48).into_boxed_str()),
            valuation_policy_hash: Box::leak("44".repeat(32).into_boxed_str()),
            source_manifest_hash: Box::leak("55".repeat(48).into_boxed_str()),
            source_id: "aave",
            source_domain: "eip155:42161",
            asset_or_position_id: position_static,
            reserve_owner_commitment: owner_commitment,
            quantity_verifier_commitment: policy_static,
            valuation_verifier_commitment: policy_static,
            observed_at_pftl_height: 200,
            expected_gross_assets: 200_000_000,
            expected_total_liabilities: 0,
            expected_evidence_commitment: Box::leak("00".repeat(48).into_boxed_str()),
        };
        let statement = aave_v3_owner_authorization_statement_v1(&proof, &placeholder).unwrap();
        let digest = eip191_hash_message(&statement);
        let (signature, recovery_id) = owner_key
            .sign_prehash_recoverable(digest.as_slice())
            .unwrap();
        proof.ownership_signature = Signature::from((signature, recovery_id))
            .as_bytes()
            .to_vec();
        let evidence = Box::leak(proof.commitment().unwrap().into_boxed_str());
        let context = AaveV3VerifyContextV1 {
            expected_evidence_commitment: evidence,
            ..placeholder
        };
        (proof, context)
    }

    #[test]
    fn verifies_full_aave_checkpoint_state_owner_and_guest_dispatch() {
        let (proof, context) = synthetic_fixture();
        let verified = verify_aave_v3_proof_v1(&proof, &context).unwrap();
        assert_eq!(verified.collateral_usd_e8, 200_000_000);
        assert_eq!(verified.liability_usd_e8, 0);

        let proof_context = ReserveProofContextV1 {
            pftl_genesis_hash: context.pftl_genesis_hash.to_string(),
            nav_asset_id: context.nav_asset_id.to_string(),
            proof_profile_id: context.proof_profile_id.to_string(),
            valuation_policy_hash: context.valuation_policy_hash.to_string(),
            source_manifest_hash: context.source_manifest_hash.to_string(),
            valuation_unit_id: "66".repeat(48),
            valuation_scale: 100_000_000,
            observation_epoch: 1,
            observation_not_before: 199,
            observation_not_after: 200,
        };
        let entry = SourceManifestEntryV1 {
            source_id: context.source_id.to_string(),
            adapter_kind: AAVE_V3_ADAPTER_KIND_V1.to_string(),
            source_domain: context.source_domain.to_string(),
            asset_or_position_id: context.asset_or_position_id.to_string(),
            reserve_owner_commitment: context.reserve_owner_commitment.to_string(),
            quantity_verifier_commitment: context.quantity_verifier_commitment.to_string(),
            valuation_verifier_commitment: context.valuation_verifier_commitment.to_string(),
            quantity_evidence_class: TrustClassV1::Cryptographic,
            valuation_evidence_class: TrustClassV1::Cryptographic,
            freshness_policy: FreshnessPolicyV1 {
                max_age_blocks: 10,
                max_observation_span_blocks: 10,
            },
            haircut_policy_hash: "77".repeat(48),
            liability_treatment: LiabilityTreatmentV1::Asset,
            adapter_schema_version: 1,
        };
        let evidence = SourceEvidenceV1::AaveV3 {
            evidence_commitment: context.expected_evidence_commitment.to_string(),
            proof: Box::new(proof),
        };
        let observation = SourceObservationV1 {
            source_id: context.source_id.to_string(),
            observed_at_block: 200,
            gross_assets: 200_000_000,
            total_liabilities: 0,
            quantity_evidence: evidence.clone(),
            valuation_evidence: evidence,
            disclosure_commitment: "88".repeat(48),
        };
        verify_observation_evidence(
            &proof_context,
            &entry,
            &observation,
            EvidenceDimensionV1::Quantity,
        )
        .unwrap();
        verify_observation_evidence(
            &proof_context,
            &entry,
            &observation,
            EvidenceDimensionV1::Valuation,
        )
        .unwrap();
    }

    #[test]
    fn rejects_owner_policy_value_and_position_substitution() {
        let (proof, context) = synthetic_fixture();

        let mut bad_owner = proof.clone();
        bad_owner.ownership_signature[0] ^= 1;
        let evidence = Box::leak(bad_owner.commitment().unwrap().into_boxed_str());
        let bad_context = AaveV3VerifyContextV1 {
            expected_evidence_commitment: evidence,
            ..context
        };
        assert_eq!(
            verify_aave_v3_proof_v1(&bad_owner, &bad_context),
            Err(AaveV3Error::OwnerAuthorization)
        );

        let (mut omitted, context) = synthetic_fixture();
        omitted.positions.clear();
        assert_eq!(
            verify_aave_v3_proof_v1(&omitted, &context),
            Err(AaveV3Error::BoundsExceeded)
        );

        let (mut substituted_slot, context) = synthetic_fixture();
        substituted_slot.positions[0].user_state_slot_index += U256::from(1u8);
        let evidence = Box::leak(substituted_slot.commitment().unwrap().into_boxed_str());
        let bad_context = AaveV3VerifyContextV1 {
            expected_evidence_commitment: evidence,
            ..context
        };
        assert_eq!(
            verify_aave_v3_proof_v1(&substituted_slot, &bad_context),
            Err(AaveV3Error::PositionMismatch)
        );

        let (proof, mut context) = synthetic_fixture();
        context.expected_gross_assets += 1;
        assert_eq!(
            verify_aave_v3_proof_v1(&proof, &context),
            Err(AaveV3Error::EvidenceCommitment)
        );
    }

    #[test]
    fn historical_aave_state_reconstructs_collateral_and_debt() {
        let historical: HistoricalWitness = serde_json::from_str(HISTORICAL_WITNESS).unwrap();
        let collateral_policy = AaveV3PositionPolicyV1 {
            position_id: "collateral-weth".to_string(),
            kind: AaveV3PositionKindV1::Collateral,
            underlying_asset: historical.collateral.underlying_asset,
            token_address: historical.collateral.token_account.address,
            token_code_hash: historical.collateral.token_account.code_hash,
            user_state_slot_index: historical.collateral.user_state_slot_index,
            decimals: historical.collateral.decimals,
            chainlink_proxy_code_hash: historical
                .collateral
                .oracle
                .chainlink
                .proxy_account
                .code_hash,
            chainlink_aggregator_code_hash: historical
                .collateral
                .oracle
                .chainlink
                .aggregator_account
                .code_hash,
            oracle_source: AaveOracleSourcePolicyV1::DirectChainlink {
                proxy_address: historical.collateral.oracle.chainlink.proxy_account.address,
            },
        };
        let debt_source = match &historical.debt.oracle.source_kind {
            HistoricalOracleSourceProof::CappedStable {
                adapter_account, ..
            } => AaveOracleSourcePolicyV1::CappedStable {
                adapter_address: adapter_account.address,
                adapter_code_hash: adapter_account.code_hash,
                chainlink_proxy_address: historical.debt.oracle.chainlink.proxy_account.address,
                price_cap_slot_index: 1,
            },
            HistoricalOracleSourceProof::DirectChainlink => {
                panic!("historical debt source changed")
            }
        };
        let debt_policy = AaveV3PositionPolicyV1 {
            position_id: "debt-usdc".to_string(),
            kind: AaveV3PositionKindV1::VariableDebt,
            underlying_asset: historical.debt.underlying_asset,
            token_address: historical.debt.token_account.address,
            token_code_hash: historical.debt.token_account.code_hash,
            user_state_slot_index: historical.debt.user_state_slot_index,
            decimals: historical.debt.decimals,
            chainlink_proxy_code_hash: historical.debt.oracle.chainlink.proxy_account.code_hash,
            chainlink_aggregator_code_hash: historical
                .debt
                .oracle
                .chainlink
                .aggregator_account
                .code_hash,
            oracle_source: debt_source,
        };
        let reserve_slot = historical
            .collateral
            .reserve
            .reserve_mapping_slot_index
            .to::<u64>();
        assert_eq!(
            historical.debt.reserve.reserve_mapping_slot_index,
            U256::from(reserve_slot)
        );
        let policy = AaveV3PolicyV1 {
            source_domain: "eip155:42161".to_string(),
            ethereum_chain_id: 42_161,
            pool_address: historical.collateral.reserve.pool_account.address,
            pool_code_hash: historical.collateral.reserve.pool_account.code_hash,
            oracle_address: historical.collateral.oracle.aave_oracle_account.address,
            oracle_code_hash: historical.collateral.oracle.aave_oracle_account.code_hash,
            reserve_mapping_slot_index: reserve_slot,
            oracle_sources_slot_index: 0,
            chainlink_proxy_phase_slot_index: 2,
            chainlink_hot_vars_slot_index: 13,
            chainlink_transmissions_slot_index: 17,
            seconds_per_year: 31_536_000,
            max_oracle_age_seconds: 86_400,
            positions: vec![collateral_policy.clone(), debt_policy.clone()],
        };
        let collateral = position_proof("collateral-weth", historical.collateral);
        let debt = position_proof("debt-usdc", historical.debt);
        let collateral_value = verify_position(
            &policy,
            historical.owner,
            &collateral,
            &collateral_policy,
            historical.state_root,
            historical.block_timestamp,
        )
        .unwrap();
        let debt_value = verify_position(
            &policy,
            historical.owner,
            &debt,
            &debt_policy,
            historical.state_root,
            historical.block_timestamp,
        )
        .unwrap();
        assert_eq!(collateral_value, 57_251_478_898);
        assert_eq!(debt_value, 20_084_880_217);
    }

    #[test]
    fn transmission_freshness_and_rounding_fail_closed() {
        let value = U256::from_be_slice(
            &hex::decode("6a2c63376a2c632a0000000000000000000000000000000000000026c5712331")
                .unwrap(),
        );
        let (answer, observed, transmitted) = transmission_answer_and_timestamps(value).unwrap();
        assert_eq!(answer, 166_521_283_377);
        assert!(transmitted >= observed);
        assert_eq!(ray_mul_floor(1, AAVE_RAY + 1).unwrap(), 1);
        assert_eq!(ray_mul_ceil(1, AAVE_RAY + 1).unwrap(), 2);
        assert_eq!(usd_e8_floor(1, 1, 1).unwrap(), 0);
        assert_eq!(usd_e8_ceil(1, 1, 1).unwrap(), 1);
        assert_eq!(checked_pow10(39), Err(AaveV3Error::ArithmeticOverflow));
    }
}
