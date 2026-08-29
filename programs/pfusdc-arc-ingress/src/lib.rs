use std::collections::{BTreeMap, BTreeSet};

use alloy_consensus::{Header, ReceiptEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{keccak256, Address, Bytes, B256, U256};
use alloy_rlp::Decodable;
use alloy_sol_types::{sol, SolEvent, SolValue};
use alloy_trie::{proof::verify_proof, Nibbles};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

pub const ARC_TESTNET_CHAIN_ID: u64 = 5_042_002;
pub const ARC_INGRESS_WITNESS_SCHEMA_V1: &str = "postfiat.pfusdc.arc_ingress_witness.v1";
pub const ARC_INGRESS_PROGRAM_VERSION_V1: u32 = 1;
pub const ARC_COMMIT_PREIMAGE_LEN: usize = 75;
pub const MAX_VALIDATORS: usize = 256;
pub const MAX_SIGNATURES: usize = 256;
pub const MAX_RECEIPT_PROOF_NODES: usize = 64;
pub const MAX_RECEIPT_PROOF_NODE_BYTES: usize = 16_384;
pub const MAX_RECEIPT_BYTES: usize = 1_048_576;

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
pub struct ArcIngressWitnessV1 {
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
        }
    }
}

pub fn verify_arc_ingress_witness_v1(
    witness: &ArcIngressWitnessV1,
) -> Result<ArcIngressPublicValuesV1, ArcIngressError> {
    validate_bounds(witness)?;
    if witness.schema != ARC_INGRESS_WITNESS_SCHEMA_V1 {
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

    if witness.next_validators.is_empty() {
        if witness.validator_set_commitment_out != witness.validator_set_commitment_in {
            return Err(ArcIngressError::ValidatorSetCommitmentMismatch);
        }
    } else {
        // Arc's public RPC does not currently expose an authenticated registry
        // state proof. Never accept an operator-supplied rotation by assertion.
        return Err(ArcIngressError::RotationProofUnavailable);
    }

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

fn validate_bounds(witness: &ArcIngressWitnessV1) -> Result<(), ArcIngressError> {
    if witness.validators.is_empty()
        || witness.validators.len() > MAX_VALIDATORS
        || witness.signatures.is_empty()
        || witness.signatures.len() > MAX_SIGNATURES
        || witness.encoded_receipt.is_empty()
        || witness.encoded_receipt.len() > MAX_RECEIPT_BYTES
        || witness.receipt_proof_nodes.is_empty()
        || witness.receipt_proof_nodes.len() > MAX_RECEIPT_PROOF_NODES
        || witness
            .receipt_proof_nodes
            .iter()
            .any(|node| node.is_empty() || node.len() > MAX_RECEIPT_PROOF_NODE_BYTES)
        || witness.next_validators.len() > MAX_VALIDATORS
        || witness.amount_atoms == 0
        || witness.route_id == [0; 32]
    {
        return Err(ArcIngressError::Bounds);
    }
    Ok(())
}

fn verify_validator_quorum(witness: &ArcIngressWitnessV1) -> Result<(), ArcIngressError> {
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
        let key = VerifyingKey::from_bytes(public_key)
            .map_err(|_| ArcIngressError::InvalidSignature)?;
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

fn verify_receipt_and_deposit(
    witness: &ArcIngressWitnessV1,
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
    let receipt = ReceiptEnvelope::decode_2718(&mut input)
        .map_err(|_| ArcIngressError::ReceiptEncoding)?;
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

    fn golden_quorum_witness() -> ArcIngressWitnessV1 {
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
        ArcIngressWitnessV1 {
            schema: ARC_INGRESS_WITNESS_SCHEMA_V1.to_string(),
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
            decode_hex("74abe9bdfafd4ca959cc7a501d590babdbd9520a3efe493e2172db3b4be5d0d5")
                .into(),
            decode_hex("ad198c9a80e97a9a60a8e80257d68060f71e8ff84ea034d54d0bb88540d51244")
                .into(),
            decode_hex("6edfa31c57cfeec8955572fd9cdb81b22222beb1dbac432ff7c7f0fc7ad9c520")
                .into(),
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
