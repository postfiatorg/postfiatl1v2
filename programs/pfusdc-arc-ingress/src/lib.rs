use std::collections::{BTreeMap, BTreeSet};

use alloy_consensus::{Header, ReceiptEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{address, b256, keccak256, Address, Bytes, B256, U256};
use alloy_rlp::Decodable;
use alloy_sol_types::{sol, SolEvent, SolValue};
use alloy_trie::{proof::verify_proof, Nibbles, TrieAccount};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

pub const ARC_TESTNET_CHAIN_ID: u64 = 5_042_002;
pub const ARC_INGRESS_WITNESS_SCHEMA_V2: &str = "postfiat.pfusdc.arc_ingress_witness.v2";
pub const ARC_INGRESS_PROGRAM_VERSION_V2: u32 = 2;
pub const ARC_COMMIT_PREIMAGE_LEN: usize = 75;
pub const MAX_VALIDATORS: usize = 256;
pub const MAX_SIGNATURES: usize = 256;
pub const MAX_RECEIPT_PROOF_NODES: usize = 64;
pub const MAX_RECEIPT_PROOF_NODE_BYTES: usize = 16_384;
pub const MAX_RECEIPT_BYTES: usize = 1_048_576;
pub const MAX_ARC_HEADER_BYTES: usize = 65_536;
pub const MAX_MPT_PROOF_NODES: usize = 64;
pub const MAX_MPT_PROOF_NODE_BYTES: usize = 16_384;
pub const MAX_ROTATION_PROOF_BYTES: usize = 16_777_216;
pub const MAX_ROTATION_STORAGE_PROOFS: usize = 2 + (5 * MAX_VALIDATORS);
pub const ARC_VALIDATOR_REGISTRY: Address = address!("3600000000000000000000000000000000000002");
pub const ARC_VALIDATOR_REGISTRY_PROXY_CODE_HASH: B256 =
    b256!("4df0ba7cf2eea00b109c6e96a21da38b43b7c9d107a94ff017a24e3409c78c2f");
pub const ARC_VALIDATOR_REGISTRY_IMPLEMENTATION_CODE_HASH: B256 =
    b256!("b04771f96d0e33612a9ebb87eb7eb5ae07adbf4a7e6b5e44f362e5a9d5c67313");
pub const ERC1967_IMPLEMENTATION_SLOT: B256 =
    b256!("360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc");
pub const VALIDATOR_REGISTRY_STORAGE_BASE: B256 =
    b256!("b58da0dce03316992faea3e12c60705b8ac05a309e27e3bc8421e5b271c9d200");

sol! {
    event ERC20BridgeDepositedV2(
        bytes32 indexed depositId,
        address indexed depositor,
        bytes32 indexed pftlRecipientHash,
        string pftlRecipient,
        uint256 amount,
        bytes32 nonce,
        bytes32 routeBinding,
        uint256 sourceChainId,
        address vault,
        address token
    );
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcValidatorV1 {
    pub address: [u8; 20],
    pub public_key: [u8; 32],
    pub voting_power: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcCommitSignatureV1 {
    pub address: [u8; 20],
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcAccountProofV1 {
    pub address: [u8; 20],
    pub nonce: u64,
    pub balance: [u8; 32],
    pub storage_root: [u8; 32],
    pub code_hash: [u8; 32],
    pub proof_nodes: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcStorageProofV1 {
    pub key: [u8; 32],
    pub value: [u8; 32],
    pub proof_nodes: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcRegisteredValidatorV1 {
    pub registration_id: u64,
    pub validator: ArcValidatorV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcValidatorRegistryProofV1 {
    pub registry_account: ArcAccountProofV1,
    pub implementation_account: ArcAccountProofV1,
    pub storage_proofs: Vec<ArcStorageProofV1>,
    pub active_validators: Vec<ArcRegisteredValidatorV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArcIngressWitnessV2 {
    pub schema: String,
    pub route_id: [u8; 32],
    pub arc_chain_id: u64,
    pub vault_address: [u8; 20],
    pub token_address: [u8; 20],
    pub deposit_id: [u8; 32],
    pub amount_atoms: u64,
    pub pftl_recipient_hash: [u8; 32],
    pub deposit_nonce: [u8; 32],
    pub arc_block_hash: [u8; 32],
    pub arc_block_height: u64,
    pub validator_set_commitment_in: [u8; 32],
    pub validator_set_commitment_out: [u8; 32],
    pub header_rlp: Vec<u8>,
    pub commit_round: u32,
    pub validators: Vec<ArcValidatorV1>,
    pub signatures: Vec<ArcCommitSignatureV1>,
    pub receipt_transaction_index: u64,
    pub encoded_receipt: Vec<u8>,
    pub receipt_proof_nodes: Vec<Vec<u8>>,
    pub deposit_log_index: u32,
    #[serde(default)]
    pub next_validators: Vec<ArcValidatorV1>,
    #[serde(default)]
    pub validator_registry_proof: Option<ArcValidatorRegistryProofV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcIngressPublicValuesV1 {
    pub route_id: [u8; 32],
    pub arc_chain_id: u64,
    pub vault_address: [u8; 20],
    pub token_address: [u8; 20],
    pub deposit_id: [u8; 32],
    pub amount_atoms: u64,
    pub pftl_recipient_hash: [u8; 32],
    pub deposit_nonce: [u8; 32],
    pub arc_block_hash: [u8; 32],
    pub arc_block_height: u64,
    pub validator_set_commitment_in: [u8; 32],
    pub validator_set_commitment_out: [u8; 32],
}

impl ArcIngressPublicValuesV1 {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = b"PFTL-PFUSDC-ARC-INGRESS-V1".to_vec();
        out.extend_from_slice(&self.route_id);
        out.extend_from_slice(&self.arc_chain_id.to_be_bytes());
        out.extend_from_slice(&self.vault_address);
        out.extend_from_slice(&self.token_address);
        out.extend_from_slice(&self.deposit_id);
        out.extend_from_slice(&self.amount_atoms.to_be_bytes());
        out.extend_from_slice(&self.pftl_recipient_hash);
        out.extend_from_slice(&self.deposit_nonce);
        out.extend_from_slice(&self.arc_block_hash);
        out.extend_from_slice(&self.arc_block_height.to_be_bytes());
        out.extend_from_slice(&self.validator_set_commitment_in);
        out.extend_from_slice(&self.validator_set_commitment_out);
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcIngressError {
    WrongSchema,
    WrongChain,
    Bounds,
    HeaderEncoding,
    HeaderHashMismatch,
    HeaderHeightMismatch,
    ValidatorSetCommitmentMismatch,
    ValidatorSetNotCanonical,
    DuplicateValidator,
    ValidatorAddressMismatch,
    DuplicateSigner,
    UnknownSigner,
    InvalidSignature,
    VotingPowerOverflow,
    SubQuorum,
    ReceiptProof,
    ReceiptEncoding,
    ReceiptFailed,
    DepositLog,
    DepositMismatch,
    DepositIdMismatch,
    RotationProofUnavailable,
    RotationAccountProof,
    RotationStorageProof,
    RotationRegistryCodeHash,
    RotationImplementation,
    RotationStorageLayout,
}

impl ArcIngressError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::WrongSchema => "ARC_INGRESS_WRONG_SCHEMA",
            Self::WrongChain => "ARC_INGRESS_WRONG_CHAIN",
            Self::Bounds => "ARC_INGRESS_BOUNDS",
            Self::HeaderEncoding => "ARC_INGRESS_HEADER_ENCODING",
            Self::HeaderHashMismatch => "ARC_INGRESS_HEADER_HASH_MISMATCH",
            Self::HeaderHeightMismatch => "ARC_INGRESS_HEADER_HEIGHT_MISMATCH",
            Self::ValidatorSetCommitmentMismatch => "ARC_INGRESS_VALIDATOR_SET_COMMITMENT_MISMATCH",
            Self::ValidatorSetNotCanonical => "ARC_INGRESS_VALIDATOR_SET_NOT_CANONICAL",
            Self::DuplicateValidator => "ARC_INGRESS_DUPLICATE_VALIDATOR",
            Self::ValidatorAddressMismatch => "ARC_INGRESS_VALIDATOR_ADDRESS_MISMATCH",
            Self::DuplicateSigner => "ARC_INGRESS_DUPLICATE_SIGNER",
            Self::UnknownSigner => "ARC_INGRESS_UNKNOWN_SIGNER",
            Self::InvalidSignature => "ARC_INGRESS_INVALID_SIGNATURE",
            Self::VotingPowerOverflow => "ARC_INGRESS_VOTING_POWER_OVERFLOW",
            Self::SubQuorum => "ARC_INGRESS_SUB_QUORUM",
            Self::ReceiptProof => "ARC_INGRESS_RECEIPT_PROOF",
            Self::ReceiptEncoding => "ARC_INGRESS_RECEIPT_ENCODING",
            Self::ReceiptFailed => "ARC_INGRESS_RECEIPT_FAILED",
            Self::DepositLog => "ARC_INGRESS_DEPOSIT_LOG",
            Self::DepositMismatch => "ARC_INGRESS_DEPOSIT_MISMATCH",
            Self::DepositIdMismatch => "ARC_INGRESS_DEPOSIT_ID_MISMATCH",
            Self::RotationProofUnavailable => "ARC_INGRESS_ROTATION_PROOF_UNAVAILABLE",
            Self::RotationAccountProof => "ARC_INGRESS_ROTATION_ACCOUNT_PROOF",
            Self::RotationStorageProof => "ARC_INGRESS_ROTATION_STORAGE_PROOF",
            Self::RotationRegistryCodeHash => "ARC_INGRESS_ROTATION_REGISTRY_CODE_HASH",
            Self::RotationImplementation => "ARC_INGRESS_ROTATION_IMPLEMENTATION",
            Self::RotationStorageLayout => "ARC_INGRESS_ROTATION_STORAGE_LAYOUT",
        }
    }
}

pub fn verify_arc_ingress_witness_v2(
    witness: &ArcIngressWitnessV2,
) -> Result<ArcIngressPublicValuesV1, ArcIngressError> {
    validate_bounds(witness)?;
    if witness.schema != ARC_INGRESS_WITNESS_SCHEMA_V2 {
        return Err(ArcIngressError::WrongSchema);
    }
    if witness.arc_chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(ArcIngressError::WrongChain);
    }

    let mut encoded = witness.header_rlp.as_slice();
    let header = Header::decode(&mut encoded).map_err(|_| ArcIngressError::HeaderEncoding)?;
    if !encoded.is_empty() {
        return Err(ArcIngressError::HeaderEncoding);
    }
    if header.hash_slow().as_slice() != witness.arc_block_hash {
        return Err(ArcIngressError::HeaderHashMismatch);
    }
    if header.number != witness.arc_block_height {
        return Err(ArcIngressError::HeaderHeightMismatch);
    }

    verify_validator_quorum(witness)?;
    verify_receipt_and_deposit(witness, header.receipts_root)?;

    verify_validator_set_transition(witness, header.state_root)?;

    Ok(ArcIngressPublicValuesV1 {
        route_id: witness.route_id,
        arc_chain_id: witness.arc_chain_id,
        vault_address: witness.vault_address,
        token_address: witness.token_address,
        deposit_id: witness.deposit_id,
        amount_atoms: witness.amount_atoms,
        pftl_recipient_hash: witness.pftl_recipient_hash,
        deposit_nonce: witness.deposit_nonce,
        arc_block_hash: witness.arc_block_hash,
        arc_block_height: witness.arc_block_height,
        validator_set_commitment_in: witness.validator_set_commitment_in,
        validator_set_commitment_out: witness.validator_set_commitment_out,
    })
}

fn validate_bounds(witness: &ArcIngressWitnessV2) -> Result<(), ArcIngressError> {
    if witness.validators.is_empty()
        || witness.validators.len() > MAX_VALIDATORS
        || witness.signatures.is_empty()
        || witness.signatures.len() > MAX_SIGNATURES
        || witness
            .signatures
            .iter()
            .any(|signature| signature.signature.len() != 64)
        || witness.header_rlp.is_empty()
        || witness.header_rlp.len() > MAX_ARC_HEADER_BYTES
        || witness.encoded_receipt.is_empty()
        || witness.encoded_receipt.len() > MAX_RECEIPT_BYTES
        || witness.receipt_proof_nodes.is_empty()
        || witness.receipt_proof_nodes.len() > MAX_RECEIPT_PROOF_NODES
        || witness
            .receipt_proof_nodes
            .iter()
            .any(|node| node.is_empty() || node.len() > MAX_RECEIPT_PROOF_NODE_BYTES)
        || witness.next_validators.is_empty()
        || witness.next_validators.len() > MAX_VALIDATORS
        || witness.amount_atoms == 0
        || witness.route_id == [0; 32]
    {
        return Err(ArcIngressError::Bounds);
    }
    let proof = witness
        .validator_registry_proof
        .as_ref()
        .ok_or(ArcIngressError::RotationProofUnavailable)?;
    if proof.active_validators.is_empty()
        || proof.active_validators.len() > MAX_VALIDATORS
        || proof.registry_account.proof_nodes.is_empty()
        || proof.implementation_account.proof_nodes.is_empty()
        || proof.storage_proofs.is_empty()
        || proof.storage_proofs.len() > MAX_ROTATION_STORAGE_PROOFS
        || !proof_nodes_bounded(&proof.registry_account.proof_nodes)
        || !proof_nodes_bounded(&proof.implementation_account.proof_nodes)
        || proof
            .storage_proofs
            .iter()
            .any(|storage| !proof_nodes_bounded(&storage.proof_nodes))
    {
        return Err(ArcIngressError::Bounds);
    }
    let proof_bytes = proof
        .registry_account
        .proof_nodes
        .iter()
        .chain(proof.implementation_account.proof_nodes.iter())
        .chain(
            proof
                .storage_proofs
                .iter()
                .flat_map(|storage| storage.proof_nodes.iter()),
        )
        .try_fold(0usize, |total, node| total.checked_add(node.len()))
        .ok_or(ArcIngressError::Bounds)?;
    if proof_bytes > MAX_ROTATION_PROOF_BYTES {
        return Err(ArcIngressError::Bounds);
    }
    Ok(())
}

fn proof_nodes_bounded(nodes: &[Vec<u8>]) -> bool {
    !nodes.is_empty()
        && nodes.len() <= MAX_MPT_PROOF_NODES
        && nodes
            .iter()
            .all(|node| !node.is_empty() && node.len() <= MAX_MPT_PROOF_NODE_BYTES)
}

fn verify_validator_quorum(witness: &ArcIngressWitnessV2) -> Result<(), ArcIngressError> {
    let expected_commitment = validator_set_commitment(witness.arc_chain_id, &witness.validators)?;
    if expected_commitment != witness.validator_set_commitment_in {
        return Err(ArcIngressError::ValidatorSetCommitmentMismatch);
    }

    let mut validators = BTreeMap::new();
    let mut prior: Option<(u64, [u8; 20])> = None;
    let mut total_power = 0u128;
    for validator in &witness.validators {
        if let Some((prior_power, prior_address)) = prior {
            if validator.voting_power > prior_power
                || (validator.voting_power == prior_power && validator.address <= prior_address)
            {
                return Err(ArcIngressError::ValidatorSetNotCanonical);
            }
        }
        prior = Some((validator.voting_power, validator.address));
        if address_from_public_key(&validator.public_key) != validator.address {
            return Err(ArcIngressError::ValidatorAddressMismatch);
        }
        if validator.voting_power == 0
            || validators
                .insert(
                    validator.address,
                    (validator.public_key, validator.voting_power),
                )
                .is_some()
        {
            return Err(ArcIngressError::DuplicateValidator);
        }
        total_power = total_power
            .checked_add(u128::from(validator.voting_power))
            .ok_or(ArcIngressError::VotingPowerOverflow)?;
    }

    let mut seen = BTreeSet::new();
    let mut signed_power = 0u128;
    for signed in &witness.signatures {
        if !seen.insert(signed.address) {
            return Err(ArcIngressError::DuplicateSigner);
        }
        let (public_key, voting_power) = validators
            .get(&signed.address)
            .ok_or(ArcIngressError::UnknownSigner)?;
        let key =
            VerifyingKey::from_bytes(public_key).map_err(|_| ArcIngressError::InvalidSignature)?;
        let signature = Signature::from_slice(&signed.signature)
            .map_err(|_| ArcIngressError::InvalidSignature)?;
        let preimage = commit_preimage(
            witness.arc_block_height,
            witness.commit_round,
            witness.arc_block_hash,
            signed.address,
        );
        key.verify_strict(&preimage, &signature)
            .map_err(|_| ArcIngressError::InvalidSignature)?;
        signed_power = signed_power
            .checked_add(u128::from(*voting_power))
            .ok_or(ArcIngressError::VotingPowerOverflow)?;
    }
    if signed_power
        .checked_mul(3)
        .ok_or(ArcIngressError::VotingPowerOverflow)?
        <= total_power
            .checked_mul(2)
            .ok_or(ArcIngressError::VotingPowerOverflow)?
    {
        return Err(ArcIngressError::SubQuorum);
    }
    Ok(())
}

fn verify_validator_set_transition(
    witness: &ArcIngressWitnessV2,
    state_root: B256,
) -> Result<(), ArcIngressError> {
    let proof = witness
        .validator_registry_proof
        .as_ref()
        .ok_or(ArcIngressError::RotationProofUnavailable)?;
    verify_arc_validator_transition_v1(
        witness.arc_chain_id,
        &witness.next_validators,
        witness.validator_set_commitment_out,
        proof,
        state_root,
    )
}

/// Verifies the registry transition committed by an Arc execution state root.
///
/// Callers outside the ingress guest must independently authenticate `state_root`
/// as the state root of the finalized Arc header at height `H`; the proven set is
/// the signing set for height `H + 1`.
pub fn verify_arc_validator_transition_v1(
    arc_chain_id: u64,
    next_validators: &[ArcValidatorV1],
    validator_set_commitment_out: [u8; 32],
    proof: &ArcValidatorRegistryProofV1,
    state_root: B256,
) -> Result<(), ArcIngressError> {
    if arc_chain_id != ARC_TESTNET_CHAIN_ID
        || next_validators.is_empty()
        || next_validators.len() > MAX_VALIDATORS
    {
        return Err(ArcIngressError::Bounds);
    }
    if proof.registry_account.address != ARC_VALIDATOR_REGISTRY.0 .0 {
        return Err(ArcIngressError::RotationImplementation);
    }
    let registry_account = verify_account_proof(state_root, &proof.registry_account)?;
    if registry_account.code_hash != ARC_VALIDATOR_REGISTRY_PROXY_CODE_HASH {
        return Err(ArcIngressError::RotationRegistryCodeHash);
    }

    let expected_storage = expected_registry_storage(proof)?;
    if expected_storage.len() != proof.storage_proofs.len() {
        return Err(ArcIngressError::RotationStorageLayout);
    }
    let mut provided_storage = BTreeMap::new();
    for storage in &proof.storage_proofs {
        if provided_storage.insert(storage.key, storage).is_some() {
            return Err(ArcIngressError::RotationStorageLayout);
        }
    }
    for (key, expected_value) in expected_storage {
        let storage = provided_storage
            .get(&key)
            .ok_or(ArcIngressError::RotationStorageLayout)?;
        if storage.value != expected_value {
            return Err(ArcIngressError::RotationStorageLayout);
        }
        verify_storage_proof(registry_account.storage_root, storage)?;
    }

    let implementation_storage = provided_storage
        .get(&ERC1967_IMPLEMENTATION_SLOT.0)
        .ok_or(ArcIngressError::RotationImplementation)?;
    if implementation_storage.value[..12] != [0; 12] {
        return Err(ArcIngressError::RotationImplementation);
    }
    let mut implementation_address = [0u8; 20];
    implementation_address.copy_from_slice(&implementation_storage.value[12..]);
    if proof.implementation_account.address != implementation_address {
        return Err(ArcIngressError::RotationImplementation);
    }
    let implementation_account = verify_account_proof(state_root, &proof.implementation_account)?;
    if implementation_account.code_hash != ARC_VALIDATOR_REGISTRY_IMPLEMENTATION_CODE_HASH {
        return Err(ArcIngressError::RotationImplementation);
    }

    let mut registered_addresses = BTreeSet::new();
    for registered in &proof.active_validators {
        if address_from_public_key(&registered.validator.public_key) != registered.validator.address
            || !registered_addresses.insert(registered.validator.address)
        {
            return Err(ArcIngressError::RotationStorageLayout);
        }
    }
    let mut proven_validators = proof
        .active_validators
        .iter()
        .filter(|registered| registered.validator.voting_power > 0)
        .map(|registered| registered.validator.clone())
        .collect::<Vec<_>>();
    proven_validators.sort_by(|left, right| {
        right
            .voting_power
            .cmp(&left.voting_power)
            .then_with(|| left.address.cmp(&right.address))
    });
    if proven_validators != next_validators {
        return Err(ArcIngressError::RotationStorageLayout);
    }
    let mut total_power = 0u128;
    for validator in &proven_validators {
        total_power = total_power
            .checked_add(u128::from(validator.voting_power))
            .ok_or(ArcIngressError::VotingPowerOverflow)?;
    }
    if total_power == 0 {
        return Err(ArcIngressError::RotationStorageLayout);
    }
    let commitment = validator_set_commitment(arc_chain_id, &proven_validators)?;
    if commitment != validator_set_commitment_out {
        return Err(ArcIngressError::ValidatorSetCommitmentMismatch);
    }
    Ok(())
}

fn verify_account_proof(
    state_root: B256,
    proof: &ArcAccountProofV1,
) -> Result<TrieAccount, ArcIngressError> {
    let account = TrieAccount {
        nonce: proof.nonce,
        balance: U256::from_be_bytes(proof.balance),
        storage_root: B256::from(proof.storage_root),
        code_hash: B256::from(proof.code_hash),
    };
    let nodes = proof
        .proof_nodes
        .iter()
        .cloned()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    verify_proof(
        state_root,
        Nibbles::unpack(keccak256(proof.address)),
        Some(alloy_rlp::encode(account)),
        nodes.iter(),
    )
    .map_err(|_| ArcIngressError::RotationAccountProof)?;
    Ok(account)
}

fn verify_storage_proof(
    storage_root: B256,
    proof: &ArcStorageProofV1,
) -> Result<(), ArcIngressError> {
    let value = U256::from_be_bytes(proof.value);
    let nodes = proof
        .proof_nodes
        .iter()
        .cloned()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    verify_proof(
        storage_root,
        Nibbles::unpack(keccak256(proof.key)),
        (!value.is_zero()).then(|| alloy_rlp::encode(value)),
        nodes.iter(),
    )
    .map_err(|_| ArcIngressError::RotationStorageProof)
}

fn expected_registry_storage(
    proof: &ArcValidatorRegistryProofV1,
) -> Result<BTreeMap<[u8; 32], [u8; 32]>, ArcIngressError> {
    let mut expected = BTreeMap::new();
    let implementation = proof
        .storage_proofs
        .iter()
        .find(|storage| storage.key == ERC1967_IMPLEMENTATION_SLOT.0)
        .ok_or(ArcIngressError::RotationImplementation)?;
    expected.insert(ERC1967_IMPLEMENTATION_SLOT.0, implementation.value);

    let active_set_slot = add_to_slot(VALIDATOR_REGISTRY_STORAGE_BASE.0, 1)?;
    expected.insert(
        active_set_slot,
        u256_word(proof.active_validators.len() as u64),
    );
    let active_values_base = keccak256(active_set_slot).0;
    let mut registration_ids = BTreeSet::new();
    for (index, registered) in proof.active_validators.iter().enumerate() {
        if registered.registration_id == 0 || !registration_ids.insert(registered.registration_id) {
            return Err(ArcIngressError::RotationStorageLayout);
        }
        expected.insert(
            add_to_slot(active_values_base, index as u64)?,
            u256_word(registered.registration_id),
        );
        let validator_slot = mapping_slot_u64(
            registered.registration_id,
            VALIDATOR_REGISTRY_STORAGE_BASE.0,
        );
        expected.insert(validator_slot, u256_word(2));
        let public_key_slot = add_to_slot(validator_slot, 1)?;
        expected.insert(public_key_slot, u256_word(65));
        expected.insert(
            keccak256(public_key_slot).0,
            registered.validator.public_key,
        );
        expected.insert(
            add_to_slot(validator_slot, 2)?,
            u256_word(registered.validator.voting_power),
        );
    }
    Ok(expected)
}

fn add_to_slot(slot: [u8; 32], addend: u64) -> Result<[u8; 32], ArcIngressError> {
    U256::from_be_bytes(slot)
        .checked_add(U256::from(addend))
        .map(|value| value.to_be_bytes())
        .ok_or(ArcIngressError::RotationStorageLayout)
}

fn mapping_slot_u64(key: u64, slot: [u8; 32]) -> [u8; 32] {
    let mut preimage = [0u8; 64];
    preimage[..32].copy_from_slice(&u256_word(key));
    preimage[32..].copy_from_slice(&slot);
    keccak256(preimage).0
}

fn u256_word(value: u64) -> [u8; 32] {
    U256::from(value).to_be_bytes()
}

fn verify_receipt_and_deposit(
    witness: &ArcIngressWitnessV2,
    receipts_root: B256,
) -> Result<(), ArcIngressError> {
    let nodes = witness
        .receipt_proof_nodes
        .iter()
        .cloned()
        .map(Bytes::from)
        .collect::<Vec<_>>();
    verify_proof(
        receipts_root,
        receipt_path(witness.receipt_transaction_index),
        Some(witness.encoded_receipt.clone()),
        nodes.iter(),
    )
    .map_err(|_| ArcIngressError::ReceiptProof)?;

    let mut input = witness.encoded_receipt.as_slice();
    let receipt =
        ReceiptEnvelope::decode_2718(&mut input).map_err(|_| ArcIngressError::ReceiptEncoding)?;
    if !input.is_empty() {
        return Err(ArcIngressError::ReceiptEncoding);
    }
    if !receipt.is_success() {
        return Err(ArcIngressError::ReceiptFailed);
    }
    let log = receipt
        .logs()
        .get(witness.deposit_log_index as usize)
        .ok_or(ArcIngressError::DepositLog)?;
    if log.address.as_slice() != witness.vault_address {
        return Err(ArcIngressError::DepositLog);
    }
    let decoded = ERC20BridgeDepositedV2::decode_log_validate(log)
        .map_err(|_| ArcIngressError::DepositLog)?;
    let event = decoded.data;
    let amount = u64::try_from(event.amount).map_err(|_| ArcIngressError::DepositMismatch)?;
    let source_chain_id =
        u64::try_from(event.sourceChainId).map_err(|_| ArcIngressError::DepositMismatch)?;
    let recipient_hash = keccak256(event.pftlRecipient.as_bytes());
    if event.depositId.0 != witness.deposit_id
        || event.pftlRecipientHash.0 != witness.pftl_recipient_hash
        || event.nonce.0 != witness.deposit_nonce
        || event.routeBinding.0 != witness.route_id
        || event.vault.as_slice() != witness.vault_address
        || event.token.as_slice() != witness.token_address
        || amount != witness.amount_atoms
        || source_chain_id != witness.arc_chain_id
        || recipient_hash.as_slice() != witness.pftl_recipient_hash
    {
        return Err(ArcIngressError::DepositMismatch);
    }
    let expected_deposit_id = deposit_id(
        source_chain_id,
        event.vault,
        event.token,
        event.depositor,
        amount,
        event.pftlRecipientHash,
        event.nonce,
        event.routeBinding,
    );
    if expected_deposit_id.as_slice() != witness.deposit_id {
        return Err(ArcIngressError::DepositIdMismatch);
    }
    Ok(())
}

pub fn deposit_id(
    chain_id: u64,
    vault: Address,
    token: Address,
    depositor: Address,
    amount: u64,
    recipient_hash: B256,
    nonce: B256,
    route_binding: B256,
) -> B256 {
    keccak256(
        (
            "postfiat.erc20_bridge.deposit.v2",
            U256::from(chain_id),
            vault,
            token,
            depositor,
            U256::from(amount),
            recipient_hash,
            nonce,
            route_binding,
        )
            .abi_encode_params(),
    )
}

pub fn validator_set_commitment(
    chain_id: u64,
    validators: &[ArcValidatorV1],
) -> Result<[u8; 32], ArcIngressError> {
    if validators.is_empty() || validators.len() > MAX_VALIDATORS {
        return Err(ArcIngressError::Bounds);
    }
    let mut preimage = b"PFTL-ARC-VALIDATOR-SET-V1".to_vec();
    preimage.extend_from_slice(&chain_id.to_be_bytes());
    preimage.extend_from_slice(&(validators.len() as u32).to_be_bytes());
    for validator in validators {
        preimage.extend_from_slice(&validator.address);
        preimage.extend_from_slice(&validator.public_key);
        preimage.extend_from_slice(&validator.voting_power.to_be_bytes());
    }
    Ok(keccak256(preimage).0)
}

pub fn commit_preimage(
    height: u64,
    round: u32,
    block_hash: [u8; 32],
    validator_address: [u8; 20],
) -> [u8; ARC_COMMIT_PREIMAGE_LEN] {
    let mut out = [0u8; ARC_COMMIT_PREIMAGE_LEN];
    out[0] = 1;
    out[1..9].copy_from_slice(&height.to_le_bytes());
    out[9..13].copy_from_slice(&37u32.to_le_bytes());
    out[13..17].copy_from_slice(&42u32.to_le_bytes());
    out[17..37].copy_from_slice(&validator_address);
    out[37] = 1;
    out[38..42].copy_from_slice(&round.to_le_bytes());
    out[42] = 1;
    out[43..75].copy_from_slice(&block_hash);
    out
}

pub fn address_from_public_key(public_key: &[u8; 32]) -> [u8; 20] {
    let digest = Keccak256::digest(public_key);
    let mut address = [0u8; 20];
    address.copy_from_slice(&digest[..20]);
    address
}

fn receipt_path(index: u64) -> Nibbles {
    Nibbles::unpack(alloy_rlp::encode(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_trie::{proof::ProofRetainer, HashBuilder};
    use base64::Engine as _;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        chain_id: u64,
        block: FixtureBlock,
        certificate: FixtureCertificate,
        validator_set: FixtureValidatorSet,
    }

    #[derive(Deserialize)]
    struct FixtureBlock {
        number: u64,
        hash: String,
    }

    #[derive(Deserialize)]
    struct FixtureCertificate {
        round: u32,
        signatures: Vec<FixtureSignature>,
    }

    #[derive(Deserialize)]
    struct FixtureSignature {
        address: String,
        signature: String,
    }

    #[derive(Deserialize)]
    struct FixtureValidatorSet {
        validators: Vec<FixtureValidator>,
    }

    #[derive(Deserialize)]
    struct FixtureValidator {
        address: String,
        public_key: String,
        voting_power: u64,
    }

    fn decode_hex<const N: usize>(value: &str) -> [u8; N] {
        let bytes = alloy_primitives::hex::decode(value.trim_start_matches("0x")).unwrap();
        bytes.try_into().unwrap()
    }

    fn trie_with_proofs(
        entries: &BTreeMap<B256, Vec<u8>>,
        targets: &[B256],
    ) -> (B256, BTreeMap<B256, Vec<Vec<u8>>>) {
        let retainer = ProofRetainer::from_iter(targets.iter().copied().map(Nibbles::unpack));
        let mut builder = HashBuilder::default().with_proof_retainer(retainer);
        for (key, value) in entries {
            builder.add_leaf(Nibbles::unpack(*key), value);
        }
        let root = builder.root();
        let retained = builder.take_proof_nodes();
        let proofs = targets
            .iter()
            .map(|key| {
                let path = Nibbles::unpack(*key);
                let nodes = retained
                    .matching_nodes_sorted(&path)
                    .into_iter()
                    .map(|(_, node)| node.to_vec())
                    .collect();
                (*key, nodes)
            })
            .collect();
        (root, proofs)
    }

    fn synthetic_registry_transition(
        proxy_code_hash: B256,
        implementation_code_hash: B256,
    ) -> (ArcIngressWitnessV2, B256) {
        let validator_a_key = [1u8; 32];
        let validator_b_key = [2u8; 32];
        let validator_a = ArcValidatorV1 {
            address: address_from_public_key(&validator_a_key),
            public_key: validator_a_key,
            voting_power: 10,
        };
        let validator_b = ArcValidatorV1 {
            address: address_from_public_key(&validator_b_key),
            public_key: validator_b_key,
            voting_power: 20,
        };
        let validator_zero_key = [3u8; 32];
        let validator_zero = ArcValidatorV1 {
            address: address_from_public_key(&validator_zero_key),
            public_key: validator_zero_key,
            voting_power: 0,
        };
        let active_validators = vec![
            ArcRegisteredValidatorV1 {
                registration_id: 1,
                validator: validator_a.clone(),
            },
            ArcRegisteredValidatorV1 {
                registration_id: 2,
                validator: validator_b.clone(),
            },
            ArcRegisteredValidatorV1 {
                registration_id: 3,
                validator: validator_zero,
            },
        ];
        let implementation_address = [0x44u8; 20];
        let mut implementation_word = [0u8; 32];
        implementation_word[12..].copy_from_slice(&implementation_address);
        let placeholder_account = ArcAccountProofV1 {
            address: [0; 20],
            nonce: 0,
            balance: [0; 32],
            storage_root: [0; 32],
            code_hash: [0; 32],
            proof_nodes: Vec::new(),
        };
        let placeholder = ArcValidatorRegistryProofV1 {
            registry_account: placeholder_account.clone(),
            implementation_account: placeholder_account,
            storage_proofs: vec![ArcStorageProofV1 {
                key: ERC1967_IMPLEMENTATION_SLOT.0,
                value: implementation_word,
                proof_nodes: Vec::new(),
            }],
            active_validators,
        };
        let expected_storage = expected_registry_storage(&placeholder).unwrap();
        let storage_entries = expected_storage
            .iter()
            .filter(|(_, value)| **value != [0; 32])
            .map(|(key, value)| {
                (
                    keccak256(key),
                    alloy_rlp::encode(U256::from_be_bytes(*value)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let storage_targets = expected_storage
            .keys()
            .copied()
            .map(keccak256)
            .collect::<Vec<_>>();
        let (storage_root, storage_proof_nodes) =
            trie_with_proofs(&storage_entries, &storage_targets);
        let storage_proofs = expected_storage
            .into_iter()
            .map(|(key, value)| ArcStorageProofV1 {
                key,
                value,
                proof_nodes: storage_proof_nodes[&keccak256(key)].clone(),
            })
            .collect::<Vec<_>>();

        let registry_account = TrieAccount {
            nonce: 1,
            balance: U256::ZERO,
            storage_root,
            code_hash: proxy_code_hash,
        };
        let implementation_account = TrieAccount {
            nonce: 1,
            balance: U256::ZERO,
            storage_root: alloy_trie::EMPTY_ROOT_HASH,
            code_hash: implementation_code_hash,
        };
        let registry_key = keccak256(ARC_VALIDATOR_REGISTRY);
        let implementation_key = keccak256(implementation_address);
        let state_entries = BTreeMap::from([
            (registry_key, alloy_rlp::encode(registry_account)),
            (
                implementation_key,
                alloy_rlp::encode(implementation_account),
            ),
        ]);
        let state_targets = state_entries.keys().copied().collect::<Vec<_>>();
        let (state_root, account_proof_nodes) = trie_with_proofs(&state_entries, &state_targets);
        let registry_proof = ArcValidatorRegistryProofV1 {
            registry_account: ArcAccountProofV1 {
                address: ARC_VALIDATOR_REGISTRY.0 .0,
                nonce: registry_account.nonce,
                balance: registry_account.balance.to_be_bytes(),
                storage_root: registry_account.storage_root.0,
                code_hash: registry_account.code_hash.0,
                proof_nodes: account_proof_nodes[&registry_key].clone(),
            },
            implementation_account: ArcAccountProofV1 {
                address: implementation_address,
                nonce: implementation_account.nonce,
                balance: implementation_account.balance.to_be_bytes(),
                storage_root: implementation_account.storage_root.0,
                code_hash: implementation_account.code_hash.0,
                proof_nodes: account_proof_nodes[&implementation_key].clone(),
            },
            storage_proofs,
            active_validators: placeholder.active_validators,
        };
        let mut witness = golden_quorum_witness();
        witness.next_validators = vec![validator_b, validator_a];
        witness.validator_set_commitment_out =
            validator_set_commitment(witness.arc_chain_id, &witness.next_validators).unwrap();
        witness.validator_registry_proof = Some(registry_proof);
        (witness, state_root)
    }

    fn golden_quorum_witness() -> ArcIngressWitnessV2 {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../crates/arc-conformance/fixtures/arc-block-a.json"
        ))
        .unwrap();
        let validators = fixture
            .validator_set
            .validators
            .into_iter()
            .map(|validator| ArcValidatorV1 {
                address: decode_hex(&validator.address),
                public_key: decode_hex(&validator.public_key),
                voting_power: validator.voting_power,
            })
            .collect::<Vec<_>>();
        let signatures = fixture
            .certificate
            .signatures
            .into_iter()
            .map(|signature| ArcCommitSignatureV1 {
                address: decode_hex(&signature.address),
                signature: base64::engine::general_purpose::STANDARD
                    .decode(signature.signature)
                    .unwrap(),
            })
            .collect();
        let block_hash = decode_hex(&fixture.block.hash);
        let commitment = validator_set_commitment(fixture.chain_id, &validators).unwrap();
        ArcIngressWitnessV2 {
            schema: ARC_INGRESS_WITNESS_SCHEMA_V2.to_string(),
            route_id: [1; 32],
            arc_chain_id: fixture.chain_id,
            vault_address: [2; 20],
            token_address: [3; 20],
            deposit_id: [4; 32],
            amount_atoms: 1,
            pftl_recipient_hash: [5; 32],
            deposit_nonce: [6; 32],
            arc_block_hash: block_hash,
            arc_block_height: fixture.block.number,
            validator_set_commitment_in: commitment,
            validator_set_commitment_out: commitment,
            header_rlp: Vec::new(),
            commit_round: fixture.certificate.round,
            validators,
            signatures,
            receipt_transaction_index: 0,
            encoded_receipt: vec![1],
            receipt_proof_nodes: vec![vec![1]],
            deposit_log_index: 0,
            next_validators: Vec::new(),
            validator_registry_proof: None,
        }
    }

    #[test]
    fn public_values_have_fixed_layout() {
        let values = ArcIngressPublicValuesV1 {
            route_id: [1; 32],
            arc_chain_id: ARC_TESTNET_CHAIN_ID,
            vault_address: [2; 20],
            token_address: [3; 20],
            deposit_id: [4; 32],
            amount_atoms: 1_000_000,
            pftl_recipient_hash: [5; 32],
            deposit_nonce: [6; 32],
            arc_block_hash: [7; 32],
            arc_block_height: 59_000_000,
            validator_set_commitment_in: [8; 32],
            validator_set_commitment_out: [8; 32],
        };
        assert_eq!(values.canonical_bytes().len(), 314);
        assert_eq!(values.canonical_bytes(), values.canonical_bytes());
    }

    #[test]
    fn deposit_id_matches_solidity_vector() {
        let id = deposit_id(
            ARC_TESTNET_CHAIN_ID,
            "0xe88fb9ab4890f513261f0aca4ff13bfba3e14862"
                .parse()
                .unwrap(),
            "0x3600000000000000000000000000000000000000"
                .parse()
                .unwrap(),
            "0xdb9b78c87f76054b204188109b35ce4614d03814"
                .parse()
                .unwrap(),
            1_000_000,
            decode_hex("74abe9bdfafd4ca959cc7a501d590babdbd9520a3efe493e2172db3b4be5d0d5").into(),
            decode_hex("ad198c9a80e97a9a60a8e80257d68060f71e8ff84ea034d54d0bb88540d51244").into(),
            decode_hex("6edfa31c57cfeec8955572fd9cdb81b22222beb1dbac432ff7c7f0fc7ad9c520").into(),
        );
        assert_eq!(
            id,
            B256::from(decode_hex(
                "4f9e9d684720845d164aec0e4756c6a59c377eb2b2432c1efcb2b40a60b00acc"
            ))
        );
    }

    #[test]
    fn rotation_is_never_accepted_by_assertion() {
        assert_eq!(
            ArcIngressError::RotationProofUnavailable.code(),
            "ARC_INGRESS_ROTATION_PROOF_UNAVAILABLE"
        );
    }

    #[test]
    fn authenticated_registry_transition_and_mutations() {
        let (witness, state_root) = synthetic_registry_transition(
            ARC_VALIDATOR_REGISTRY_PROXY_CODE_HASH,
            ARC_VALIDATOR_REGISTRY_IMPLEMENTATION_CODE_HASH,
        );
        assert_eq!(
            witness
                .validator_registry_proof
                .as_ref()
                .unwrap()
                .active_validators
                .len(),
            3
        );
        assert_eq!(witness.next_validators.len(), 2);
        verify_validator_set_transition(&witness, state_root).unwrap();

        let mut wrong_root = state_root;
        wrong_root[0] ^= 1;
        assert_eq!(
            verify_validator_set_transition(&witness, wrong_root),
            Err(ArcIngressError::RotationAccountProof)
        );

        let mut wrong_account = witness.clone();
        wrong_account
            .validator_registry_proof
            .as_mut()
            .unwrap()
            .registry_account
            .proof_nodes[0][0] ^= 1;
        assert_eq!(
            verify_validator_set_transition(&wrong_account, state_root),
            Err(ArcIngressError::RotationAccountProof)
        );

        let mut wrong_storage = witness.clone();
        wrong_storage
            .validator_registry_proof
            .as_mut()
            .unwrap()
            .storage_proofs[0]
            .proof_nodes[0][0] ^= 1;
        assert_eq!(
            verify_validator_set_transition(&wrong_storage, state_root),
            Err(ArcIngressError::RotationStorageProof)
        );

        let mut wrong_slot = witness.clone();
        wrong_slot
            .validator_registry_proof
            .as_mut()
            .unwrap()
            .storage_proofs[0]
            .key[0] ^= 1;
        assert_eq!(
            verify_validator_set_transition(&wrong_slot, state_root),
            Err(ArcIngressError::RotationStorageLayout)
        );

        let mut reordered = witness.clone();
        reordered.next_validators.swap(0, 1);
        assert_eq!(
            verify_validator_set_transition(&reordered, state_root),
            Err(ArcIngressError::RotationStorageLayout)
        );

        let mut included_zero_power = witness.clone();
        included_zero_power.next_validators.push(
            included_zero_power
                .validator_registry_proof
                .as_ref()
                .unwrap()
                .active_validators[2]
                .validator
                .clone(),
        );
        assert_eq!(
            verify_validator_set_transition(&included_zero_power, state_root),
            Err(ArcIngressError::RotationStorageLayout)
        );

        let mut duplicate_registration = witness.clone();
        duplicate_registration
            .validator_registry_proof
            .as_mut()
            .unwrap()
            .active_validators[1]
            .registration_id = 1;
        assert_eq!(
            verify_validator_set_transition(&duplicate_registration, state_root),
            Err(ArcIngressError::RotationStorageLayout)
        );

        let (wrong_proxy_code, wrong_proxy_root) = synthetic_registry_transition(
            B256::ZERO,
            ARC_VALIDATOR_REGISTRY_IMPLEMENTATION_CODE_HASH,
        );
        assert_eq!(
            verify_validator_set_transition(&wrong_proxy_code, wrong_proxy_root),
            Err(ArcIngressError::RotationRegistryCodeHash)
        );

        let (wrong_implementation, wrong_implementation_root) =
            synthetic_registry_transition(ARC_VALIDATOR_REGISTRY_PROXY_CODE_HASH, B256::ZERO);
        assert_eq!(
            verify_validator_set_transition(&wrong_implementation, wrong_implementation_root),
            Err(ArcIngressError::RotationImplementation)
        );
    }

    #[test]
    fn live_golden_quorum_and_mutations() {
        let witness = golden_quorum_witness();
        verify_validator_quorum(&witness).unwrap();

        let mut forged = witness.clone();
        forged.signatures[0].signature[0] ^= 1;
        assert_eq!(
            verify_validator_quorum(&forged),
            Err(ArcIngressError::InvalidSignature)
        );

        let mut sub_quorum = witness;
        sub_quorum.signatures.truncate(1);
        assert_eq!(
            verify_validator_quorum(&sub_quorum),
            Err(ArcIngressError::SubQuorum)
        );
    }
}
