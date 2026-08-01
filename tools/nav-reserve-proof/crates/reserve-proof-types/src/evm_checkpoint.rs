//! Provider-neutral EVM ERC-20 balance proofs under a governed, quorum-signed
//! state-root checkpoint.

use alloy_primitives::{keccak256, Address, Bytes, Signature, B256, U256};
use alloy_rlp::{encode, encode_fixed_size};
use alloy_trie::{nybbles::Nibbles, proof::verify_proof, TrieAccount};
use postfiat_crypto_provider::{
    ml_dsa_65_verify_with_context, ML_DSA_65_PUBLIC_KEY_BYTES, ML_DSA_65_SIGNATURE_BYTES,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};

pub const EVM_STATE_CHECKPOINT_SIGNATURE_CONTEXT_V1: &[u8] =
    b"postfiat-l1-v2/evm-reserve-state-checkpoint/v1";
pub const EVM_ERC20_ADAPTER_KIND_V1: &str = "evm-erc20-bft-checkpoint-mpt-v1";
pub const MAX_EVM_CHECKPOINT_VALIDATORS: usize = 64;
pub const MAX_EVM_PROOF_NODES: usize = 64;
pub const MAX_EVM_PROOF_NODE_BYTES: usize = 64 * 1024;
pub const MAX_EVM_PROOF_TOTAL_BYTES: usize = 512 * 1024;

const COMMITTEE_ROOT_DOMAIN: &[u8] = b"postfiat.reserve_evm_checkpoint_committee.v1";
const CHECKPOINT_DOMAIN: &[u8] = b"postfiat.reserve_evm_state_checkpoint.v1";
const EVIDENCE_DOMAIN: &[u8] = b"postfiat.reserve_evm_erc20_balance_evidence.v1";
const OWNER_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_evm_owner_commitment.v1";
const OWNER_AUTHORIZATION_DOMAIN: &[u8] = b"postfiat.reserve_evm_owner_authorization.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmCheckpointValidatorV1 {
    pub validator_id: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmCheckpointCommitteeV1 {
    pub epoch: u64,
    pub quorum: u16,
    pub validators: Vec<EvmCheckpointValidatorV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmStateCheckpointV1 {
    pub pftl_genesis_hash: String,
    pub source_domain: String,
    pub ethereum_chain_id: u64,
    pub block_number: u64,
    pub block_hash: B256,
    pub state_root: B256,
    pub observed_head_number: u64,
    pub minimum_confirmations: u32,
    pub pftl_observation_height: u64,
    pub committee_epoch: u64,
    pub committee_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmStateCheckpointVoteV1 {
    pub validator_id: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmStateCheckpointCertificateV1 {
    pub committee: EvmCheckpointCommitteeV1,
    pub checkpoint: EvmStateCheckpointV1,
    pub votes: Vec<EvmStateCheckpointVoteV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmAccountProofV1 {
    pub address: Address,
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: B256,
    pub code_hash: B256,
    pub proof: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmStorageProofV1 {
    pub key: B256,
    pub value: U256,
    pub proof: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmErc20BalanceProofV1 {
    pub checkpoint_certificate: EvmStateCheckpointCertificateV1,
    pub owner: Address,
    /// EIP-191 signature by `owner` over the NAVCoin/profile/manifest/source
    /// authorization statement constructed by `verify`.
    pub ownership_signature: Vec<u8>,
    pub token: Address,
    pub balance_slot_index: U256,
    pub token_account: EvmAccountProofV1,
    pub balance: EvmStorageProofV1,
}

impl EvmCheckpointCommitteeV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.epoch == 0
            || self.validators.is_empty()
            || self.validators.len() > MAX_EVM_CHECKPOINT_VALIDATORS
            || usize::from(self.quorum) == 0
            || usize::from(self.quorum) > self.validators.len()
        {
            return Err("EVM checkpoint committee bounds are invalid".to_string());
        }
        let mut previous: Option<&str> = None;
        for validator in &self.validators {
            validate_identifier("EVM checkpoint validator_id", &validator.validator_id)?;
            if validator.public_key.len() != ML_DSA_65_PUBLIC_KEY_BYTES {
                return Err("EVM checkpoint validator ML-DSA-65 key length is invalid".to_string());
            }
            if let Some(previous) = previous {
                if validator.validator_id.as_str() <= previous {
                    return Err(
                        "EVM checkpoint validators must be strictly sorted and unique".to_string(),
                    );
                }
            }
            previous = Some(&validator.validator_id);
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

impl EvmStateCheckpointV1 {
    fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        validate_hex("checkpoint pftl_genesis_hash", &self.pftl_genesis_hash, 48)?;
        validate_identifier("checkpoint source_domain", &self.source_domain)?;
        validate_hex("checkpoint committee_root", &self.committee_root, 48)?;
        if self.ethereum_chain_id == 0
            || self.block_number == 0
            || self.block_hash == B256::ZERO
            || self.state_root == B256::ZERO
            || self.observed_head_number == 0
            || self.minimum_confirmations == 0
            || self.pftl_observation_height == 0
            || self.committee_epoch == 0
        {
            return Err("EVM state checkpoint contains a zero required field".to_string());
        }
        let required_head = self
            .block_number
            .checked_add(u64::from(self.minimum_confirmations))
            .ok_or_else(|| "EVM checkpoint confirmation height overflows".to_string())?;
        if self.observed_head_number < required_head {
            return Err("EVM state checkpoint is below its confirmation depth".to_string());
        }
        let mut bytes = Vec::new();
        append_hex(&mut bytes, &self.pftl_genesis_hash, 48)?;
        append_bytes(&mut bytes, self.source_domain.as_bytes())?;
        bytes.extend_from_slice(&self.ethereum_chain_id.to_be_bytes());
        bytes.extend_from_slice(&self.block_number.to_be_bytes());
        bytes.extend_from_slice(self.block_hash.as_slice());
        bytes.extend_from_slice(self.state_root.as_slice());
        bytes.extend_from_slice(&self.observed_head_number.to_be_bytes());
        bytes.extend_from_slice(&self.minimum_confirmations.to_be_bytes());
        bytes.extend_from_slice(&self.pftl_observation_height.to_be_bytes());
        bytes.extend_from_slice(&self.committee_epoch.to_be_bytes());
        append_hex(&mut bytes, &self.committee_root, 48)?;
        Ok(domain_message(CHECKPOINT_DOMAIN, &bytes))
    }

    /// Return the exact statement a named committee member must sign for a
    /// checkpoint vote. This is public so independently operated validators
    /// do not need to duplicate (and potentially drift from) the guest's
    /// canonical encoding.
    pub fn vote_signing_statement(&self, validator_id: &str) -> Result<Vec<u8>, String> {
        validate_identifier("EVM checkpoint vote validator_id", validator_id)?;
        let mut statement = self.canonical_bytes()?;
        append_bytes(&mut statement, validator_id.as_bytes())?;
        Ok(statement)
    }
}

impl EvmStateCheckpointCertificateV1 {
    pub fn verify(&self) -> Result<(), String> {
        self.committee.validate()?;
        let committee_root = self.committee.root()?;
        if self.checkpoint.committee_epoch != self.committee.epoch
            || self.checkpoint.committee_root != committee_root
        {
            return Err("EVM checkpoint committee binding mismatch".to_string());
        }
        if self.votes.len() < usize::from(self.committee.quorum)
            || self.votes.len() > self.committee.validators.len()
        {
            return Err("EVM checkpoint vote count is below quorum or out of bounds".to_string());
        }
        let mut previous: Option<&str> = None;
        for vote in &self.votes {
            validate_identifier("EVM checkpoint vote validator_id", &vote.validator_id)?;
            if vote.signature.len() != ML_DSA_65_SIGNATURE_BYTES {
                return Err("EVM checkpoint vote ML-DSA-65 signature length is invalid".to_string());
            }
            if let Some(previous) = previous {
                if vote.validator_id.as_str() <= previous {
                    return Err(
                        "EVM checkpoint votes must be strictly sorted and unique".to_string()
                    );
                }
            }
            previous = Some(&vote.validator_id);
            let validator = self
                .committee
                .validators
                .iter()
                .find(|validator| validator.validator_id == vote.validator_id)
                .ok_or_else(|| "EVM checkpoint vote is from an unknown validator".to_string())?;
            let statement = self.checkpoint.vote_signing_statement(&vote.validator_id)?;
            if !ml_dsa_65_verify_with_context(
                &validator.public_key,
                &statement,
                &vote.signature,
                EVM_STATE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
            ) {
                return Err("EVM checkpoint vote signature is invalid".to_string());
            }
        }
        Ok(())
    }
}

impl EvmErc20BalanceProofV1 {
    pub fn commitment(&self) -> Result<String, String> {
        validate_proof_nodes("EVM account proof", &self.token_account.proof)?;
        validate_proof_nodes("EVM storage proof", &self.balance.proof)?;
        let mut bytes = self.checkpoint_certificate.checkpoint.canonical_bytes()?;
        append_u32(&mut bytes, self.checkpoint_certificate.votes.len())?;
        for vote in &self.checkpoint_certificate.votes {
            append_bytes(&mut bytes, vote.validator_id.as_bytes())?;
            append_bytes(&mut bytes, &vote.signature)?;
        }
        bytes.extend_from_slice(self.owner.as_slice());
        append_bytes(&mut bytes, &self.ownership_signature)?;
        bytes.extend_from_slice(self.token.as_slice());
        bytes.extend_from_slice(&self.balance_slot_index.to_be_bytes::<32>());
        append_account_proof(&mut bytes, &self.token_account)?;
        bytes.extend_from_slice(self.balance.key.as_slice());
        bytes.extend_from_slice(&self.balance.value.to_be_bytes::<32>());
        append_proof_nodes(&mut bytes, &self.balance.proof)?;
        Ok(hash48(EVIDENCE_DOMAIN, &[&bytes]))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify(
        &self,
        expected_pftl_genesis_hash: &str,
        expected_nav_asset_id: &str,
        expected_proof_profile_id: &str,
        expected_valuation_policy_hash: &str,
        expected_source_manifest_hash: &str,
        expected_source_id: &str,
        expected_source_domain: &str,
        expected_asset_or_position_id: &str,
        expected_owner_commitment: &str,
        expected_verifier_commitment: &str,
        expected_pftl_observation_height: u64,
        expected_evidence_commitment: &str,
    ) -> Result<(), String> {
        self.checkpoint_certificate.verify()?;
        let checkpoint = &self.checkpoint_certificate.checkpoint;
        if checkpoint.pftl_genesis_hash != expected_pftl_genesis_hash
            || checkpoint.source_domain != expected_source_domain
            || checkpoint.pftl_observation_height != expected_pftl_observation_height
        {
            return Err("EVM balance proof checkpoint context mismatch".to_string());
        }
        let canonical_domain = format!("eip155:{}", checkpoint.ethereum_chain_id);
        if checkpoint.source_domain != canonical_domain {
            return Err("EVM balance proof source domain is not canonical".to_string());
        }
        let canonical_position = format!("erc20:0x{}", hex::encode(self.token.as_slice()));
        if expected_asset_or_position_id != canonical_position {
            return Err("EVM balance proof token does not match the manifest position".to_string());
        }
        if evm_owner_commitment(self.owner) != expected_owner_commitment {
            return Err("EVM balance proof owner commitment mismatch".to_string());
        }
        if self.checkpoint_certificate.committee.root()? != expected_verifier_commitment {
            return Err("EVM balance proof committee commitment mismatch".to_string());
        }
        if self.commitment()? != expected_evidence_commitment {
            return Err("EVM balance proof evidence commitment mismatch".to_string());
        }
        self.verify_owner_authorization(
            expected_pftl_genesis_hash,
            expected_nav_asset_id,
            expected_proof_profile_id,
            expected_valuation_policy_hash,
            expected_source_manifest_hash,
            expected_source_id,
        )?;
        if self.token_account.address != self.token {
            return Err("EVM balance account proof is for the wrong token".to_string());
        }
        verify_account_proof(checkpoint.state_root, &self.token_account)?;
        let expected_slot = erc20_balance_slot(self.owner, self.balance_slot_index);
        if self.balance.key != expected_slot {
            return Err("EVM ERC-20 balance storage slot mismatch".to_string());
        }
        verify_storage_proof(self.token_account.storage_root, &self.balance)
    }

    fn verify_owner_authorization(
        &self,
        pftl_genesis_hash: &str,
        nav_asset_id: &str,
        proof_profile_id: &str,
        valuation_policy_hash: &str,
        source_manifest_hash: &str,
        source_id: &str,
    ) -> Result<(), String> {
        if self.ownership_signature.len() != 65 {
            return Err("EVM owner authorization signature length is invalid".to_string());
        }
        let statement = evm_owner_authorization_statement(
            pftl_genesis_hash,
            nav_asset_id,
            proof_profile_id,
            valuation_policy_hash,
            source_manifest_hash,
            source_id,
            self.owner,
            self.token,
            &self.checkpoint_certificate.checkpoint.committee_root,
        )?;
        let signature: [u8; 65] = self
            .ownership_signature
            .as_slice()
            .try_into()
            .map_err(|_| "EVM owner authorization signature length is invalid".to_string())?;
        let signature = Signature::from_raw_array(&signature)
            .map_err(|_| "EVM owner authorization signature is malformed".to_string())?;
        let recovered = signature
            .recover_address_from_msg(&statement)
            .map_err(|_| "EVM owner authorization signature is invalid".to_string())?;
        if recovered != self.owner {
            return Err("EVM owner authorization signer does not match reserve owner".to_string());
        }
        Ok(())
    }
}

pub fn evm_owner_commitment(owner: Address) -> String {
    hash48(OWNER_COMMITMENT_DOMAIN, &[owner.as_slice()])
}

pub fn erc20_balance_slot(owner: Address, slot_index: U256) -> B256 {
    let mut encoded = [0u8; 64];
    encoded[12..32].copy_from_slice(owner.as_slice());
    encoded[32..64].copy_from_slice(&slot_index.to_be_bytes::<32>());
    keccak256(encoded)
}

#[allow(clippy::too_many_arguments)]
pub fn evm_owner_authorization_statement(
    pftl_genesis_hash: &str,
    nav_asset_id: &str,
    proof_profile_id: &str,
    valuation_policy_hash: &str,
    source_manifest_hash: &str,
    source_id: &str,
    owner: Address,
    token: Address,
    committee_root: &str,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    append_hex(&mut bytes, pftl_genesis_hash, 48)?;
    append_hex(&mut bytes, nav_asset_id, 48)?;
    append_hex(&mut bytes, proof_profile_id, 48)?;
    append_hex(&mut bytes, valuation_policy_hash, 32)?;
    append_hex(&mut bytes, source_manifest_hash, 48)?;
    validate_identifier("EVM authorization source_id", source_id)?;
    append_bytes(&mut bytes, source_id.as_bytes())?;
    bytes.extend_from_slice(owner.as_slice());
    bytes.extend_from_slice(token.as_slice());
    append_hex(&mut bytes, committee_root, 48)?;
    Ok(domain_message(OWNER_AUTHORIZATION_DOMAIN, &bytes))
}

fn verify_account_proof(state_root: B256, account: &EvmAccountProofV1) -> Result<(), String> {
    validate_proof_nodes("EVM account proof", &account.proof)?;
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
        state_root,
        Nibbles::unpack(keccak256(account.address.as_slice())),
        Some(expected),
        proof.iter(),
    )
    .map_err(|_| "EVM account Merkle-Patricia proof is invalid".to_string())
}

fn verify_storage_proof(storage_root: B256, slot: &EvmStorageProofV1) -> Result<(), String> {
    validate_proof_nodes("EVM storage proof", &slot.proof)?;
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
        storage_root,
        Nibbles::unpack(keccak256(slot.key.as_slice())),
        expected,
        proof.iter(),
    )
    .map_err(|_| "EVM storage Merkle-Patricia proof is invalid".to_string())
}

fn validate_proof_nodes(label: &str, nodes: &[Vec<u8>]) -> Result<(), String> {
    if nodes.is_empty() || nodes.len() > MAX_EVM_PROOF_NODES {
        return Err(format!("{label} node count is out of bounds"));
    }
    let mut total = 0usize;
    for node in nodes {
        if node.is_empty() || node.len() > MAX_EVM_PROOF_NODE_BYTES {
            return Err(format!("{label} node length is out of bounds"));
        }
        total = total
            .checked_add(node.len())
            .ok_or_else(|| format!("{label} byte count overflows"))?;
    }
    if total > MAX_EVM_PROOF_TOTAL_BYTES {
        return Err(format!("{label} total byte length is out of bounds"));
    }
    Ok(())
}

fn append_account_proof(out: &mut Vec<u8>, account: &EvmAccountProofV1) -> Result<(), String> {
    out.extend_from_slice(account.address.as_slice());
    out.extend_from_slice(&account.nonce.to_be_bytes());
    out.extend_from_slice(&account.balance.to_be_bytes::<32>());
    out.extend_from_slice(account.storage_root.as_slice());
    out.extend_from_slice(account.code_hash.as_slice());
    append_proof_nodes(out, &account.proof)
}

fn append_proof_nodes(out: &mut Vec<u8>, nodes: &[Vec<u8>]) -> Result<(), String> {
    validate_proof_nodes("EVM proof", nodes)?;
    append_u32(out, nodes.len())?;
    for node in nodes {
        append_bytes(out, node)?;
    }
    Ok(())
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

fn validate_hex(field: &str, value: &str, expected_bytes: usize) -> Result<(), String> {
    if value.len() != expected_bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be canonical lowercase hex"));
    }
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), String> {
    validate_hex("canonical hex", value, bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| "canonical hex decode failed")?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let length = u32::try_from(value.len()).map_err(|_| "byte string length overflows u32")?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "collection length overflows u32")?;
    out.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn domain_message(domain: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + domain.len() + payload.len());
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
    use alloy_primitives::eip191_hash_message;
    use alloy_trie::{nybbles::Nibbles, proof::ProofRetainer, HashBuilder};
    use k256::ecdsa::SigningKey;
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    fn committee() -> (EvmCheckpointCommitteeV1, Vec<MlDsa65KeyPair>) {
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        let committee = EvmCheckpointCommitteeV1 {
            epoch: 7,
            quorum: 3,
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| EvmCheckpointValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        (committee, keys)
    }

    fn state_proof(
        token: Address,
        owner: Address,
        slot_index: U256,
        balance: U256,
    ) -> (B256, EvmAccountProofV1, EvmStorageProofV1) {
        let slot_key = erc20_balance_slot(owner, slot_index);
        let storage_path = Nibbles::unpack(keccak256(slot_key.as_slice()));
        let storage_value = encode_fixed_size(&balance).as_ref().to_vec();
        let mut storage_builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([storage_path]));
        storage_builder.add_leaf(storage_path, &storage_value);
        let storage_root = storage_builder.root();
        let storage_nodes = storage_builder
            .take_proof_nodes()
            .into_nodes_sorted()
            .into_iter()
            .map(|(_, node)| node.to_vec())
            .collect();

        let trie_account = TrieAccount {
            nonce: 1,
            balance: U256::ZERO,
            storage_root,
            code_hash: B256::repeat_byte(0x77),
        };
        let account_path = Nibbles::unpack(keccak256(token.as_slice()));
        let mut account_builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter([account_path]));
        account_builder.add_leaf(account_path, &encode(trie_account));
        let state_root = account_builder.root();
        let account_nodes = account_builder
            .take_proof_nodes()
            .into_nodes_sorted()
            .into_iter()
            .map(|(_, node)| node.to_vec())
            .collect();
        (
            state_root,
            EvmAccountProofV1 {
                address: token,
                nonce: trie_account.nonce,
                balance: trie_account.balance,
                storage_root,
                code_hash: trie_account.code_hash,
                proof: account_nodes,
            },
            EvmStorageProofV1 {
                key: slot_key,
                value: balance,
                proof: storage_nodes,
            },
        )
    }

    fn fixture() -> EvmErc20BalanceProofV1 {
        let ownership_key = SigningKey::from_bytes((&[0x22; 32]).into()).unwrap();
        let owner = Address::from_private_key(&ownership_key);
        let token = Address::repeat_byte(0x33);
        let slot_index = U256::from(9);
        let (state_root, token_account, balance) =
            state_proof(token, owner, slot_index, U256::from(123_456u64));
        let (committee, keys) = committee();
        let checkpoint = EvmStateCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            source_domain: "eip155:1".to_string(),
            ethereum_chain_id: 1,
            block_number: 100,
            block_hash: B256::repeat_byte(0x44),
            state_root,
            observed_head_number: 112,
            minimum_confirmations: 12,
            pftl_observation_height: 200,
            committee_epoch: committee.epoch,
            committee_root: committee.root().unwrap(),
        };
        let mut certificate = EvmStateCheckpointCertificateV1 {
            committee,
            checkpoint,
            votes: (0..3)
                .map(|index| EvmStateCheckpointVoteV1 {
                    validator_id: format!("validator-{index}"),
                    signature: vec![0; ML_DSA_65_SIGNATURE_BYTES],
                })
                .collect(),
        };
        for (index, vote) in certificate.votes.iter_mut().enumerate() {
            let statement = {
                let mut statement = certificate.checkpoint.canonical_bytes().unwrap();
                append_bytes(&mut statement, vote.validator_id.as_bytes()).unwrap();
                statement
            };
            vote.signature = ml_dsa_65_sign_with_context_seed(
                &keys[index].private_key,
                &statement,
                EVM_STATE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                &[0x80 + index as u8; 32],
            )
            .unwrap();
        }
        let mut proof = EvmErc20BalanceProofV1 {
            checkpoint_certificate: certificate,
            owner,
            ownership_signature: vec![0; 65],
            token,
            balance_slot_index: slot_index,
            token_account,
            balance,
        };
        let statement = evm_owner_authorization_statement(
            &"11".repeat(48),
            &"22".repeat(48),
            &"33".repeat(48),
            &"44".repeat(32),
            &"55".repeat(48),
            "cash-primary",
            proof.owner,
            proof.token,
            &proof.checkpoint_certificate.checkpoint.committee_root,
        )
        .unwrap();
        let digest = eip191_hash_message(&statement);
        let (signature, recovery_id) = ownership_key
            .sign_prehash_recoverable(digest.as_slice())
            .unwrap();
        proof.ownership_signature = Signature::from((signature, recovery_id))
            .as_bytes()
            .to_vec();
        proof
    }

    #[test]
    fn verifies_checkpoint_committee_owner_and_mpt_balance() {
        let proof = fixture();
        let commitment = proof.commitment().unwrap();
        proof
            .verify(
                &"11".repeat(48),
                &"22".repeat(48),
                &"33".repeat(48),
                &"44".repeat(32),
                &"55".repeat(48),
                "cash-primary",
                "eip155:1",
                &format!("erc20:0x{}", hex::encode(proof.token.as_slice())),
                &evm_owner_commitment(proof.owner),
                &proof.checkpoint_certificate.committee.root().unwrap(),
                200,
                &commitment,
            )
            .unwrap();
    }

    #[test]
    fn rejects_quorum_signature_commitment_and_storage_substitution() {
        let proof = fixture();
        let commitment = proof.commitment().unwrap();

        let mut bad_signature = proof.clone();
        bad_signature.checkpoint_certificate.votes[0].signature[0] ^= 1;
        assert!(bad_signature.checkpoint_certificate.verify().is_err());

        let mut bad_owner = proof.clone();
        bad_owner.ownership_signature[0] ^= 1;
        let bad_owner_commitment = bad_owner.commitment().unwrap();
        assert!(bad_owner
            .verify(
                &"11".repeat(48),
                &"22".repeat(48),
                &"33".repeat(48),
                &"44".repeat(32),
                &"55".repeat(48),
                "cash-primary",
                "eip155:1",
                &format!("erc20:0x{}", hex::encode(bad_owner.token.as_slice())),
                &evm_owner_commitment(bad_owner.owner),
                &bad_owner.checkpoint_certificate.committee.root().unwrap(),
                200,
                &bad_owner_commitment,
            )
            .unwrap_err()
            .contains("owner authorization"));

        assert!(proof
            .verify(
                &"11".repeat(48),
                &"22".repeat(48),
                &"33".repeat(48),
                &"44".repeat(32),
                &"55".repeat(48),
                "cash-primary",
                "eip155:1",
                &format!("erc20:0x{}", hex::encode(proof.token.as_slice())),
                &evm_owner_commitment(proof.owner),
                &proof.checkpoint_certificate.committee.root().unwrap(),
                200,
                &"00".repeat(48),
            )
            .unwrap_err()
            .contains("evidence commitment"));

        let mut tampered_balance = proof;
        tampered_balance.balance.value += U256::from(1);
        let tampered_commitment = tampered_balance.commitment().unwrap();
        assert!(tampered_balance
            .verify(
                &"11".repeat(48),
                &"22".repeat(48),
                &"33".repeat(48),
                &"44".repeat(32),
                &"55".repeat(48),
                "cash-primary",
                "eip155:1",
                &format!("erc20:0x{}", hex::encode(tampered_balance.token.as_slice())),
                &evm_owner_commitment(tampered_balance.owner),
                &tampered_balance
                    .checkpoint_certificate
                    .committee
                    .root()
                    .unwrap(),
                200,
                &tampered_commitment,
            )
            .unwrap_err()
            .contains("storage Merkle-Patricia proof"));
        assert_ne!(commitment, tampered_commitment);
    }
}
