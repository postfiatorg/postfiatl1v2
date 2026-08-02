//! Public Monero reserve-proof verification with NAV/profile replay binding,
//! transaction/RingCT verification, governed head anchoring, and certified
//! key-image spent status.

use alloy_primitives::{keccak256, B256};
use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use monero::blockdata::block::BlockHeader;
use monero::blockdata::transaction::{Transaction, TxOut};
use monero::consensus::encode::deserialize;
use monero::cryptonote::hash::Hashable;
use monero::util::ringct::{EcdhInfo, RctSigBase};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};
use thiserror::Error;

use crate::bft_checkpoint::BftSourceCheckpointCertificateV1;

pub const MONERO_RESERVE_ADAPTER_KIND_V1: &str = "monero-reserve-proof-v1";
pub const MONERO_CHECKPOINT_KIND_V1: &str = "monero-head-key-image-status-v1";
pub const XMR_RESERVE_MAX_OUTPUTS: usize = 64;
pub const XMR_RESERVE_MAX_TX_TREE_BRANCH: usize = 64;
pub const XMR_RESERVE_MAX_HEADER_LINKS: usize = 4096;
pub const XMR_RESERVE_MAX_HEADER_BYTES: usize = 256;
pub const XMR_RESERVE_MAX_TRANSACTION_BYTES: usize = 512 * 1024;
pub const XMR_RESERVE_MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_monero_policy.v1";
const OWNER_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_monero_owner.v1";
const CHALLENGE_DOMAIN: &[u8] = b"postfiat.reserve_monero_challenge.v1";
const EVIDENCE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_monero_evidence.v1";
const STATUS_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_monero_status.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct XmrReserveWitness {
    pub address_spend_public_key: B256,
    pub address_view_public_key: B256,
    pub message: String,
    pub entries: Vec<XmrReserveOutputWitness>,
    pub subaddr_spendkeys: Vec<XmrSubaddrSpendKeyProof>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct XmrReserveOutputWitness {
    pub txid: B256,
    pub index_in_tx: u64,
    pub shared_secret: B256,
    pub key_image: B256,
    pub shared_secret_sig: XmrSignature,
    pub key_image_sig: XmrSignature,
    pub transaction_bytes: Vec<u8>,
    pub tx_tree: XmrTxTreeProof,
    pub block_anchor: XmrBlockAnchor,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct XmrSubaddrSpendKeyProof {
    pub archive_marker: u64,
    pub public_key: B256,
    pub signature: XmrSignature,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct XmrSignature {
    pub c: B256,
    pub r: B256,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct XmrTxTreeProof {
    pub tx_tree_root: B256,
    pub tx_count: u64,
    pub tx_index: u64,
    pub branch: Vec<B256>,
    pub output_block_header_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub enum XmrBlockAnchor {
    PinnedOutputBlock {
        pinned_output_block_hash: B256,
    },
    HeaderChain {
        pinned_head_hash: B256,
        links: Vec<XmrBlockHeaderLink>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct XmrBlockHeaderLink {
    pub header_bytes: Vec<u8>,
    pub tx_tree_root: B256,
    pub tx_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct XmrReserveVerification {
    pub xmr_atomic: u128,
    pub key_images: Vec<B256>,
    pub output_block_hashes: Vec<B256>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MoneroReservePolicyV1 {
    pub source_domain: String,
    pub position_id: String,
    pub address_spend_public_key: B256,
    pub address_view_public_key: B256,
    pub checkpoint_committee_root: String,
    pub allow_pinned_output_blocks: bool,
    pub max_outputs: u16,
    pub max_header_links_per_output: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MoneroKeyImageStatusV1 {
    pub key_image: B256,
    pub spent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MoneroReserveProofV1 {
    pub policy: MoneroReservePolicyV1,
    pub checkpoint_certificate: BftSourceCheckpointCertificateV1,
    pub reserve: XmrReserveWitness,
    pub key_image_statuses: Vec<MoneroKeyImageStatusV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MoneroReserveVerificationV1 {
    pub xmr_atomic: u128,
    pub key_images: Vec<B256>,
    pub output_block_hashes: Vec<B256>,
    pub checkpoint_height: u64,
    pub checkpoint_hash: B256,
    pub evidence_commitment: String,
}

pub struct MoneroReserveVerifyContextV1<'a> {
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
    pub observation_epoch: u64,
    pub observation_not_before: u64,
    pub observation_not_after: u64,
    pub observed_at_pftl_height: u64,
    pub expected_evidence_commitment: &'a str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum XmrReserveLegError {
    #[error("reserve witness has no outputs or too many outputs")]
    BadEntryCount,
    #[error("reserve witness has no subaddress spend-key proof or too many proofs")]
    BadSubaddressProofCount,
    #[error("address spend key is absent from reserve proof")]
    BadAddressBinding,
    #[error("duplicate key image or output")]
    DuplicateProofItem,
    #[error("bad subaddress spend-key signature")]
    BadSubaddressSpendKeySignature,
    #[error("bad shared-secret tx proof")]
    BadSharedSecretSignature,
    #[error("bad key-image ring signature")]
    BadKeyImageSignature,
    #[error("key-image status set does not exactly match the reserve outputs")]
    BadKeyImageStatusSet,
    #[error("a reserve output key image is spent at the certified source head")]
    SpentKeyImage,
    #[error("derived subaddress spend key is absent from reserve proof")]
    BadDerivedSubaddressSpendKey,
    #[error("Monero transaction failed to decode")]
    BadTransaction,
    #[error("Monero transaction hash does not match the witness txid")]
    BadTransactionHash,
    #[error("reserve output index is out of bounds")]
    BadOutputIndex,
    #[error("reserve output has no one-time key")]
    BadOutputKey,
    #[error("Monero transaction has no tx public key")]
    MissingTxPubkey,
    #[error("RingCT amount proof failed")]
    BadRingCtAmount,
    #[error("Monero tx tree proof failed")]
    BadTxTreeProof,
    #[error("Monero block anchor failed")]
    BadBlockAnchor,
    #[error("bad Monero block header")]
    BadBlockHeader,
    #[error("arithmetic overflow")]
    ArithmeticOverflow,
    #[error("invalid curve point")]
    InvalidCurvePoint,
    #[error("unsupported reserve-proof shape")]
    UnsupportedShape,
}

pub fn verify_xmr_reserve_witness(
    witness: &XmrReserveWitness,
) -> Result<XmrReserveVerification, XmrReserveLegError> {
    if witness.entries.len() > XMR_RESERVE_MAX_OUTPUTS {
        return Err(XmrReserveLegError::BadEntryCount);
    }
    if witness.subaddr_spendkeys.is_empty()
        || witness.subaddr_spendkeys.len() > XMR_RESERVE_MAX_OUTPUTS + 1
    {
        return Err(XmrReserveLegError::BadSubaddressProofCount);
    }
    validate_no_duplicates(witness)?;
    validate_address_binding(witness)?;

    let prefix_hash = reserve_prefix_hash(witness);
    for subaddr in &witness.subaddr_spendkeys {
        if !check_signature(&prefix_hash, subaddr.public_key, &subaddr.signature)? {
            return Err(XmrReserveLegError::BadSubaddressSpendKeySignature);
        }
    }

    let mut total_atomic = 0u128;
    let mut key_images = Vec::with_capacity(witness.entries.len());
    let mut output_block_hashes = Vec::with_capacity(witness.entries.len());
    for entry in &witness.entries {
        let (amount, output_block_hash) = verify_output_entry(witness, entry, &prefix_hash)?;
        total_atomic = total_atomic
            .checked_add(u128::from(amount))
            .ok_or(XmrReserveLegError::ArithmeticOverflow)?;
        key_images.push(entry.key_image);
        output_block_hashes.push(output_block_hash);
    }

    Ok(XmrReserveVerification {
        xmr_atomic: total_atomic,
        key_images,
        output_block_hashes,
    })
}

impl MoneroReservePolicyV1 {
    pub fn validate(&self) -> Result<(), XmrReserveLegError> {
        validate_identifier(&self.source_domain)?;
        if self.source_domain != "monero:mainnet" {
            return Err(XmrReserveLegError::UnsupportedShape);
        }
        validate_identifier(&self.position_id)?;
        validate_lower_hex(&self.checkpoint_committee_root, 48)?;
        if self.address_spend_public_key == B256::ZERO
            || self.address_view_public_key == B256::ZERO
            || self.max_outputs == 0
            || usize::from(self.max_outputs) > XMR_RESERVE_MAX_OUTPUTS
            || self.max_header_links_per_output == 0
            || usize::from(self.max_header_links_per_output) > XMR_RESERVE_MAX_HEADER_LINKS
        {
            return Err(XmrReserveLegError::UnsupportedShape);
        }
        Ok(())
    }

    pub fn commitment(&self) -> Result<String, XmrReserveLegError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| XmrReserveLegError::UnsupportedShape)?;
        Ok(hash48(POLICY_COMMITMENT_DOMAIN, &[&bytes]))
    }
}

pub fn monero_reserve_owner_commitment(spend: B256, view: B256) -> String {
    hash48(
        OWNER_COMMITMENT_DOMAIN,
        &[spend.as_slice(), view.as_slice()],
    )
}

pub fn monero_reserve_challenge_v1(
    policy: &MoneroReservePolicyV1,
    context: &MoneroReserveVerifyContextV1<'_>,
) -> Result<String, XmrReserveLegError> {
    policy.validate()?;
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
    append_hex(&mut out, &policy.commitment()?, 48)?;
    out.extend_from_slice(&context.observation_epoch.to_be_bytes());
    out.extend_from_slice(&context.observation_not_before.to_be_bytes());
    out.extend_from_slice(&context.observation_not_after.to_be_bytes());
    out.extend_from_slice(&context.observed_at_pftl_height.to_be_bytes());
    let digest = keccak256(domain_message(CHALLENGE_DOMAIN, &out));
    Ok(format!(
        "postfiat-monero-reserve-v1:{}",
        hex::encode(digest)
    ))
}

pub fn verify_monero_reserve_proof_v1(
    proof: &MoneroReserveProofV1,
    context: &MoneroReserveVerifyContextV1<'_>,
) -> Result<MoneroReserveVerificationV1, XmrReserveLegError> {
    proof.policy.validate()?;
    validate_wrapper_bounds(proof)?;
    if proof.policy.source_domain != context.source_domain
        || proof.policy.position_id != context.asset_or_position_id
        || monero_reserve_owner_commitment(
            proof.policy.address_spend_public_key,
            proof.policy.address_view_public_key,
        ) != context.reserve_owner_commitment
        || proof.policy.commitment()? != context.quantity_verifier_commitment
        || proof.reserve.address_spend_public_key != proof.policy.address_spend_public_key
        || proof.reserve.address_view_public_key != proof.policy.address_view_public_key
        || proof.reserve.message != monero_reserve_challenge_v1(&proof.policy, context)?
    {
        return Err(XmrReserveLegError::BadAddressBinding);
    }
    proof
        .checkpoint_certificate
        .verify()
        .map_err(|_| XmrReserveLegError::BadBlockAnchor)?;
    let checkpoint = &proof.checkpoint_certificate.checkpoint;
    if checkpoint.pftl_genesis_hash != context.pftl_genesis_hash
        || checkpoint.checkpoint_kind != MONERO_CHECKPOINT_KIND_V1
        || checkpoint.source_domain != proof.policy.source_domain
        || checkpoint.pftl_observation_height != context.observed_at_pftl_height
        || checkpoint.committee_root != proof.policy.checkpoint_committee_root
    {
        return Err(XmrReserveLegError::BadBlockAnchor);
    }
    let verified = verify_xmr_reserve_witness(&proof.reserve)?;
    validate_key_image_statuses(&proof.key_image_statuses, &verified.key_images)?;
    for entry in &proof.reserve.entries {
        match &entry.block_anchor {
            XmrBlockAnchor::PinnedOutputBlock { .. }
                if !proof.policy.allow_pinned_output_blocks =>
            {
                return Err(XmrReserveLegError::BadBlockAnchor)
            }
            XmrBlockAnchor::HeaderChain {
                pinned_head_hash,
                links,
            } if *pinned_head_hash != checkpoint.source_block_hash
                || links.len() > usize::from(proof.policy.max_header_links_per_output) =>
            {
                return Err(XmrReserveLegError::BadBlockAnchor);
            }
            _ => {}
        }
    }
    let status_commitment = monero_status_commitment(
        checkpoint.source_block_hash,
        &verified.output_block_hashes,
        &proof.key_image_statuses,
    )?;
    if status_commitment != checkpoint.source_state_commitment {
        return Err(XmrReserveLegError::BadBlockAnchor);
    }
    let evidence_commitment = proof.evidence_commitment()?;
    if evidence_commitment != context.expected_evidence_commitment {
        return Err(XmrReserveLegError::UnsupportedShape);
    }
    Ok(MoneroReserveVerificationV1 {
        xmr_atomic: verified.xmr_atomic,
        key_images: verified.key_images,
        output_block_hashes: verified.output_block_hashes,
        checkpoint_height: checkpoint.source_height,
        checkpoint_hash: checkpoint.source_block_hash,
        evidence_commitment,
    })
}

impl MoneroReserveProofV1 {
    pub fn evidence_commitment(&self) -> Result<String, XmrReserveLegError> {
        self.policy.validate()?;
        validate_wrapper_bounds(self)?;
        let bytes = serde_json::to_vec(self).map_err(|_| XmrReserveLegError::UnsupportedShape)?;
        Ok(hash48(EVIDENCE_COMMITMENT_DOMAIN, &[&bytes]))
    }
}

pub fn monero_status_commitment(
    head_hash: B256,
    output_block_hashes: &[B256],
    statuses: &[MoneroKeyImageStatusV1],
) -> Result<B256, XmrReserveLegError> {
    if output_block_hashes.len() > XMR_RESERVE_MAX_OUTPUTS
        || statuses.len() > XMR_RESERVE_MAX_OUTPUTS
    {
        return Err(XmrReserveLegError::BadEntryCount);
    }
    let mut out = Vec::new();
    out.extend_from_slice(head_hash.as_slice());
    append_u32(&mut out, output_block_hashes.len())?;
    for hash in output_block_hashes {
        out.extend_from_slice(hash.as_slice());
    }
    append_u32(&mut out, statuses.len())?;
    let mut previous = None;
    for status in statuses {
        if previous >= Some(status.key_image) {
            return Err(XmrReserveLegError::DuplicateProofItem);
        }
        previous = Some(status.key_image);
        out.extend_from_slice(status.key_image.as_slice());
        out.push(u8::from(status.spent));
    }
    Ok(keccak256(domain_message(STATUS_COMMITMENT_DOMAIN, &out)))
}

fn validate_wrapper_bounds(proof: &MoneroReserveProofV1) -> Result<(), XmrReserveLegError> {
    if proof.reserve.message.is_empty()
        || proof.reserve.message.len() > 256
        || proof.reserve.entries.len() > usize::from(proof.policy.max_outputs)
        || proof.key_image_statuses.len() > usize::from(proof.policy.max_outputs)
    {
        return Err(XmrReserveLegError::BadEntryCount);
    }
    let mut total = 0usize;
    for entry in &proof.reserve.entries {
        if entry.transaction_bytes.is_empty()
            || entry.transaction_bytes.len() > XMR_RESERVE_MAX_TRANSACTION_BYTES
            || entry.tx_tree.branch.len() > XMR_RESERVE_MAX_TX_TREE_BRANCH
            || entry.tx_tree.output_block_header_bytes.is_empty()
            || entry.tx_tree.output_block_header_bytes.len() > XMR_RESERVE_MAX_HEADER_BYTES
        {
            return Err(XmrReserveLegError::UnsupportedShape);
        }
        total = total
            .checked_add(entry.transaction_bytes.len())
            .and_then(|value| value.checked_add(entry.tx_tree.output_block_header_bytes.len()))
            .ok_or(XmrReserveLegError::ArithmeticOverflow)?;
        if let XmrBlockAnchor::HeaderChain { links, .. } = &entry.block_anchor {
            if links.len() > usize::from(proof.policy.max_header_links_per_output) {
                return Err(XmrReserveLegError::BadBlockAnchor);
            }
            for link in links {
                if link.header_bytes.is_empty()
                    || link.header_bytes.len() > XMR_RESERVE_MAX_HEADER_BYTES
                {
                    return Err(XmrReserveLegError::UnsupportedShape);
                }
                total = total
                    .checked_add(link.header_bytes.len())
                    .ok_or(XmrReserveLegError::ArithmeticOverflow)?;
            }
        }
    }
    if total > XMR_RESERVE_MAX_TOTAL_BYTES {
        return Err(XmrReserveLegError::UnsupportedShape);
    }
    Ok(())
}

fn validate_key_image_statuses(
    statuses: &[MoneroKeyImageStatusV1],
    verified_key_images: &[B256],
) -> Result<(), XmrReserveLegError> {
    if statuses.len() != verified_key_images.len() {
        return Err(XmrReserveLegError::BadKeyImageStatusSet);
    }
    let mut previous = None;
    for status in statuses {
        if previous >= Some(status.key_image) || !verified_key_images.contains(&status.key_image) {
            return Err(XmrReserveLegError::BadKeyImageStatusSet);
        }
        if status.spent {
            return Err(XmrReserveLegError::SpentKeyImage);
        }
        previous = Some(status.key_image);
    }
    Ok(())
}

fn verify_output_entry(
    witness: &XmrReserveWitness,
    entry: &XmrReserveOutputWitness,
    prefix_hash: &[u8; 32],
) -> Result<(u64, B256), XmrReserveLegError> {
    let tx = parse_transaction(&entry.transaction_bytes)?;
    if B256::from(tx.hash().to_bytes()) != entry.txid {
        return Err(XmrReserveLegError::BadTransactionHash);
    }
    let output = tx
        .prefix
        .outputs
        .get(usize::try_from(entry.index_in_tx).map_err(|_| XmrReserveLegError::BadOutputIndex)?)
        .ok_or(XmrReserveLegError::BadOutputIndex)?;
    let output_key = output
        .get_one_time_key()
        .ok_or(XmrReserveLegError::BadOutputKey)?
        .to_bytes();
    let tx_pubkeys = tx_pubkeys_from_extra(&tx)?;
    let selected_tx_pubkey =
        verify_entry_signatures_and_address(witness, entry, prefix_hash, &tx_pubkeys, output_key)?;
    let amount = decode_and_verify_amount(output, tx.rct_signatures.sig.as_ref(), entry)?;

    let tx_tree_root = verify_tx_tree(entry)?;
    if tx_tree_root != entry.tx_tree.tx_tree_root {
        return Err(XmrReserveLegError::BadTxTreeProof);
    }
    let output_block_hash = monero_block_hash(
        &entry.tx_tree.output_block_header_bytes,
        entry.tx_tree.tx_tree_root,
        entry.tx_tree.tx_count,
    )?;
    verify_block_anchor(entry, output_block_hash)?;

    let _ = selected_tx_pubkey;
    Ok((amount, output_block_hash))
}

fn verify_entry_signatures_and_address(
    witness: &XmrReserveWitness,
    entry: &XmrReserveOutputWitness,
    prefix_hash: &[u8; 32],
    tx_pubkeys: &[[u8; 32]],
    output_key: [u8; 32],
) -> Result<[u8; 32], XmrReserveLegError> {
    let mut selected_tx_pubkey = None;
    for tx_pubkey in tx_pubkeys {
        if check_tx_proof(
            prefix_hash,
            witness.address_view_public_key,
            B256::from(*tx_pubkey),
            entry.shared_secret,
            &entry.shared_secret_sig,
        )? {
            selected_tx_pubkey = Some(*tx_pubkey);
            break;
        }
    }
    let tx_pubkey = selected_tx_pubkey.ok_or(XmrReserveLegError::BadSharedSecretSignature)?;

    if !check_ring_signature(
        prefix_hash,
        entry.key_image,
        B256::from(output_key),
        &entry.key_image_sig,
    )? {
        return Err(XmrReserveLegError::BadKeyImageSignature);
    }

    let subaddr_spendkey =
        derive_subaddress_spend_public_key(&output_key, entry.shared_secret, entry.index_in_tx)?;
    if witness
        .subaddr_spendkeys
        .iter()
        .all(|entry| entry.public_key != B256::from(subaddr_spendkey))
    {
        return Err(XmrReserveLegError::BadDerivedSubaddressSpendKey);
    }
    Ok(tx_pubkey)
}

fn decode_and_verify_amount(
    output: &TxOut,
    rct: Option<&RctSigBase>,
    entry: &XmrReserveOutputWitness,
) -> Result<u64, XmrReserveLegError> {
    let clear_amount = *output.amount;
    if clear_amount != 0 {
        return Ok(clear_amount);
    }
    let rct = rct.ok_or(XmrReserveLegError::BadRingCtAmount)?;
    let index =
        usize::try_from(entry.index_in_tx).map_err(|_| XmrReserveLegError::BadRingCtAmount)?;
    let ecdh = rct
        .ecdh_info
        .get(index)
        .ok_or(XmrReserveLegError::BadRingCtAmount)?;
    let commitment = rct
        .out_pk
        .get(index)
        .ok_or(XmrReserveLegError::BadRingCtAmount)?
        .mask
        .key;
    let shared_scalar = derivation_to_scalar(entry.shared_secret, entry.index_in_tx)?;
    let (amount, mask) = match ecdh {
        EcdhInfo::Standard { mask, amount } => {
            let shared_sec1 = monero_keccak(shared_scalar.as_bytes());
            let shared_sec2 = monero_keccak(&shared_sec1);
            let mask_scalar =
                Scalar::from_bytes_mod_order(mask.key) - Scalar::from_bytes_mod_order(shared_sec1);
            let amount_scalar = Scalar::from_bytes_mod_order(amount.key)
                - Scalar::from_bytes_mod_order(shared_sec2);
            let amount_bytes: [u8; 8] = amount_scalar.to_bytes()[..8]
                .try_into()
                .map_err(|_| XmrReserveLegError::BadRingCtAmount)?;
            (u64::from_le_bytes(amount_bytes), mask_scalar)
        }
        EcdhInfo::Bulletproof { amount } => {
            let mut amount_key = Vec::with_capacity(6 + 32);
            amount_key.extend_from_slice(b"amount");
            amount_key.extend_from_slice(shared_scalar.as_bytes());
            let amount_mask = monero_keccak(&amount_key);
            let mut amount_bytes = amount.0;
            for (left, right) in amount_bytes.iter_mut().zip(amount_mask.iter()) {
                *left ^= *right;
            }
            let mut commitment_key = Vec::with_capacity(15 + 32);
            commitment_key.extend_from_slice(b"commitment_mask");
            commitment_key.extend_from_slice(shared_scalar.as_bytes());
            (
                u64::from_le_bytes(amount_bytes),
                hash_to_scalar(&commitment_key),
            )
        }
    };
    let expected =
        (ED25519_BASEPOINT_POINT * mask) + (*monero_generators_mirror::H * Scalar::from(amount));
    let actual = decompress_point(&commitment)?;
    if expected != actual {
        return Err(XmrReserveLegError::BadRingCtAmount);
    }
    Ok(amount)
}

fn verify_tx_tree(entry: &XmrReserveOutputWitness) -> Result<B256, XmrReserveLegError> {
    let count =
        usize::try_from(entry.tx_tree.tx_count).map_err(|_| XmrReserveLegError::BadTxTreeProof)?;
    let index =
        usize::try_from(entry.tx_tree.tx_index).map_err(|_| XmrReserveLegError::BadTxTreeProof)?;
    if count == 0 || index >= count {
        return Err(XmrReserveLegError::BadTxTreeProof);
    }
    let (path, depth) = monero_tree_path(count, index)?;
    if entry.tx_tree.branch.len() != depth {
        return Err(XmrReserveLegError::BadTxTreeProof);
    }
    Ok(verify_monero_tree_branch(
        entry.txid,
        &entry.tx_tree.branch,
        path,
    ))
}

fn verify_block_anchor(
    entry: &XmrReserveOutputWitness,
    output_block_hash: B256,
) -> Result<(), XmrReserveLegError> {
    match &entry.block_anchor {
        XmrBlockAnchor::PinnedOutputBlock {
            pinned_output_block_hash,
        } => {
            if *pinned_output_block_hash != output_block_hash {
                return Err(XmrReserveLegError::BadBlockAnchor);
            }
            Ok(())
        }
        XmrBlockAnchor::HeaderChain {
            pinned_head_hash,
            links,
        } => {
            if links.is_empty() || links.len() > XMR_RESERVE_MAX_HEADER_LINKS {
                return Err(XmrReserveLegError::BadBlockAnchor);
            }
            let mut current_hash = output_block_hash;
            for link in links {
                let prev_id = monero_header_prev_hash(&link.header_bytes)?;
                if prev_id != current_hash {
                    return Err(XmrReserveLegError::BadBlockAnchor);
                }
                current_hash =
                    monero_block_hash(&link.header_bytes, link.tx_tree_root, link.tx_count)?;
            }
            if current_hash != *pinned_head_hash {
                return Err(XmrReserveLegError::BadBlockAnchor);
            }
            Ok(())
        }
    }
}

pub fn monero_block_hash(
    header_bytes: &[u8],
    tx_tree_root: B256,
    tx_count: u64,
) -> Result<B256, XmrReserveLegError> {
    let _: BlockHeader =
        deserialize(header_bytes).map_err(|_| XmrReserveLegError::BadBlockHeader)?;
    let mut blob = Vec::with_capacity(header_bytes.len() + 32 + 10);
    blob.extend_from_slice(header_bytes);
    blob.extend_from_slice(tx_tree_root.as_slice());
    write_varint(tx_count, &mut blob);
    let mut preimage = Vec::with_capacity(blob.len() + 10);
    write_varint(
        u64::try_from(blob.len()).map_err(|_| XmrReserveLegError::ArithmeticOverflow)?,
        &mut preimage,
    );
    preimage.extend_from_slice(&blob);
    Ok(keccak256(preimage))
}

fn monero_header_prev_hash(header_bytes: &[u8]) -> Result<B256, XmrReserveLegError> {
    let header: BlockHeader =
        deserialize(header_bytes).map_err(|_| XmrReserveLegError::BadBlockHeader)?;
    Ok(B256::from(header.prev_id.to_bytes()))
}

fn check_signature(
    prefix_hash: &[u8; 32],
    pubkey: B256,
    sig: &XmrSignature,
) -> Result<bool, XmrReserveLegError> {
    let c = match canonical_scalar(&b256_array(sig.c)) {
        Some(scalar) if scalar != Scalar::ZERO => scalar,
        _ => return Ok(false),
    };
    let r = match canonical_scalar(&b256_array(sig.r)) {
        Some(scalar) => scalar,
        _ => return Ok(false),
    };
    let pub_point = decompress_point(&b256_array(pubkey))?;
    let comm = (pub_point * c) + (ED25519_BASEPOINT_POINT * r);
    let comm_bytes = comm.compress().to_bytes();
    if comm_bytes == infinity_bytes() {
        return Ok(false);
    }
    let mut buf = Vec::with_capacity(96);
    buf.extend_from_slice(prefix_hash);
    buf.extend_from_slice(pubkey.as_slice());
    buf.extend_from_slice(&comm_bytes);
    Ok(hash_to_scalar(&buf) == c)
}

fn check_tx_proof(
    prefix_hash: &[u8; 32],
    view_pubkey: B256,
    tx_pubkey: B256,
    shared_secret: B256,
    sig: &XmrSignature,
) -> Result<bool, XmrReserveLegError> {
    let c = match canonical_scalar(&b256_array(sig.c)) {
        Some(scalar) => scalar,
        _ => return Ok(false),
    };
    let r = match canonical_scalar(&b256_array(sig.r)) {
        Some(scalar) => scalar,
        _ => return Ok(false),
    };
    let r_point = decompress_point(&b256_array(view_pubkey))?;
    let a_point = decompress_point(&b256_array(tx_pubkey))?;
    let d_point = decompress_point(&b256_array(shared_secret))?;

    let x = (r_point * c) + (ED25519_BASEPOINT_POINT * r);
    let y = (d_point * c) + (a_point * r);
    let mut buf = Vec::with_capacity(32 * 9);
    buf.extend_from_slice(prefix_hash);
    buf.extend_from_slice(shared_secret.as_slice());
    buf.extend_from_slice(&x.compress().to_bytes());
    buf.extend_from_slice(&y.compress().to_bytes());
    buf.extend_from_slice(monero_keccak(b"TXPROOF_V2").as_slice());
    buf.extend_from_slice(view_pubkey.as_slice());
    buf.extend_from_slice(tx_pubkey.as_slice());
    buf.extend_from_slice(&[0u8; 32]);
    Ok(hash_to_scalar(&buf) == c)
}

fn check_ring_signature(
    prefix_hash: &[u8; 32],
    key_image: B256,
    pubkey: B256,
    sig: &XmrSignature,
) -> Result<bool, XmrReserveLegError> {
    let c = match canonical_scalar(&b256_array(sig.c)) {
        Some(scalar) => scalar,
        _ => return Ok(false),
    };
    let r = match canonical_scalar(&b256_array(sig.r)) {
        Some(scalar) => scalar,
        _ => return Ok(false),
    };
    let key_image = decompress_point(&b256_array(key_image))?;
    let pub_point = decompress_point(&b256_array(pubkey))?;
    let a = (pub_point * c) + (ED25519_BASEPOINT_POINT * r);
    let hash_point = monero_generators_mirror::hash_to_point(b256_array(pubkey));
    let b = (hash_point * r) + (key_image * c);
    let mut buf = Vec::with_capacity(96);
    buf.extend_from_slice(prefix_hash);
    buf.extend_from_slice(&a.compress().to_bytes());
    buf.extend_from_slice(&b.compress().to_bytes());
    Ok(hash_to_scalar(&buf) == c)
}

fn derive_subaddress_spend_public_key(
    output_key: &[u8; 32],
    shared_secret: B256,
    output_index: u64,
) -> Result<[u8; 32], XmrReserveLegError> {
    let scalar = derivation_to_scalar(shared_secret, output_index)?;
    let output = decompress_point(output_key)?;
    Ok((output - (ED25519_BASEPOINT_POINT * scalar))
        .compress()
        .to_bytes())
}

fn derivation_to_scalar(
    shared_secret: B256,
    output_index: u64,
) -> Result<Scalar, XmrReserveLegError> {
    let derivation = (decompress_point(&b256_array(shared_secret))? * Scalar::from(8u8))
        .compress()
        .to_bytes();
    let mut buf = Vec::with_capacity(32 + 10);
    buf.extend_from_slice(&derivation);
    write_varint(output_index, &mut buf);
    Ok(hash_to_scalar(&buf))
}

fn hash_to_scalar(data: &[u8]) -> Scalar {
    Scalar::from_bytes_mod_order(monero_keccak(data))
}

fn monero_keccak(data: &[u8]) -> [u8; 32] {
    keccak256(data).0
}

fn canonical_scalar(bytes: &[u8; 32]) -> Option<Scalar> {
    Scalar::from_canonical_bytes(*bytes).into()
}

fn decompress_point(bytes: &[u8; 32]) -> Result<EdwardsPoint, XmrReserveLegError> {
    CompressedEdwardsY(*bytes)
        .decompress()
        .ok_or(XmrReserveLegError::InvalidCurvePoint)
}

fn infinity_bytes() -> [u8; 32] {
    let mut out = [0u8; 32];
    out[0] = 1;
    out
}

fn tx_pubkeys_from_extra(tx: &Transaction) -> Result<Vec<[u8; 32]>, XmrReserveLegError> {
    let extra = tx.prefix.extra.try_parse();
    let tx_pubkey = extra
        .tx_pubkey()
        .ok_or(XmrReserveLegError::MissingTxPubkey)?;
    let mut keys = vec![tx_pubkey.to_bytes()];
    if let Some(additional) = extra.tx_additional_pubkeys() {
        keys.extend(additional.into_iter().map(|key| key.to_bytes()));
    }
    Ok(keys)
}

fn parse_transaction(bytes: &[u8]) -> Result<Transaction, XmrReserveLegError> {
    deserialize::<Transaction>(bytes).map_err(|_| XmrReserveLegError::BadTransaction)
}

fn validate_no_duplicates(witness: &XmrReserveWitness) -> Result<(), XmrReserveLegError> {
    let mut key_images = Vec::with_capacity(witness.entries.len());
    let mut outputs = Vec::with_capacity(witness.entries.len());
    for entry in &witness.entries {
        if key_images.contains(&entry.key_image)
            || outputs.contains(&(entry.txid, entry.index_in_tx))
        {
            return Err(XmrReserveLegError::DuplicateProofItem);
        }
        key_images.push(entry.key_image);
        outputs.push((entry.txid, entry.index_in_tx));
    }
    Ok(())
}

fn validate_address_binding(witness: &XmrReserveWitness) -> Result<(), XmrReserveLegError> {
    if witness
        .subaddr_spendkeys
        .iter()
        .all(|entry| entry.public_key != witness.address_spend_public_key)
    {
        return Err(XmrReserveLegError::BadAddressBinding);
    }
    Ok(())
}

fn reserve_prefix_hash(witness: &XmrReserveWitness) -> [u8; 32] {
    let mut prefix = Vec::with_capacity(witness.message.len() + 64 + witness.entries.len() * 32);
    prefix.extend_from_slice(witness.message.as_bytes());
    prefix.extend_from_slice(witness.address_spend_public_key.as_slice());
    prefix.extend_from_slice(witness.address_view_public_key.as_slice());
    for entry in &witness.entries {
        prefix.extend_from_slice(entry.key_image.as_slice());
    }
    monero_keccak(&prefix)
}

fn monero_tree_path(count: usize, mut idx: usize) -> Result<(u32, usize), XmrReserveLegError> {
    if count == 0 || idx >= count {
        return Err(XmrReserveLegError::BadTxTreeProof);
    }
    if count == 1 {
        return Ok((0, 0));
    }
    if count == 2 {
        return Ok((idx as u32, 1));
    }

    let mut path = 0u32;
    let mut depth = 0usize;
    let mut cnt = tree_hash_cnt(count);
    let initial = 2 * cnt - count;
    if idx >= initial {
        let pair_base = initial + ((idx - initial) / 2) * 2;
        let j = initial + (idx - initial) / 2;
        path = (path << 1) | if idx == pair_base { 0 } else { 1 };
        depth += 1;
        idx = j;
    }
    while cnt > 2 {
        cnt >>= 1;
        let bit = idx % 2;
        path = (path << 1) | u32::try_from(bit).map_err(|_| XmrReserveLegError::BadTxTreeProof)?;
        depth += 1;
        idx /= 2;
    }
    let bit = match idx {
        0 => 0,
        1 => 1,
        _ => return Err(XmrReserveLegError::BadTxTreeProof),
    };
    path = (path << 1) | bit;
    depth += 1;
    Ok((path, depth))
}

fn verify_monero_tree_branch(leaf: B256, siblings: &[B256], path: u32) -> B256 {
    let mut partial = leaf;
    let depth = siblings.len();
    for (d, sibling) in siblings.iter().enumerate() {
        let bit = (path >> (depth - d - 1)) & 1;
        partial = if bit == 1 {
            keccak_pair(*sibling, partial)
        } else {
            keccak_pair(partial, *sibling)
        };
    }
    partial
}

fn tree_hash_cnt(count: usize) -> usize {
    let mut pow = 2usize;
    while pow < count {
        pow <<= 1;
    }
    pow >> 1
}

fn keccak_pair(left: B256, right: B256) -> B256 {
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(left.as_slice());
    buf[32..].copy_from_slice(right.as_slice());
    keccak256(buf)
}

fn write_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn b256_array(value: B256) -> [u8; 32] {
    value.0
}

fn validate_identifier(value: &str) -> Result<(), XmrReserveLegError> {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(XmrReserveLegError::UnsupportedShape);
    }
    Ok(())
}

fn validate_lower_hex(value: &str, bytes: usize) -> Result<(), XmrReserveLegError> {
    if value.len() != bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(XmrReserveLegError::UnsupportedShape);
    }
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), XmrReserveLegError> {
    validate_lower_hex(value, bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| XmrReserveLegError::UnsupportedShape)?);
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), XmrReserveLegError> {
    let len = u32::try_from(value.len()).map_err(|_| XmrReserveLegError::ArithmeticOverflow)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), XmrReserveLegError> {
    let value = u32::try_from(value).map_err(|_| XmrReserveLegError::ArithmeticOverflow)?;
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
        BftCheckpointCommitteeV1, BftCheckpointValidatorV1, BftSourceCheckpointV1,
        BftSourceCheckpointVoteV1, BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
    };
    use monero::consensus::encode::{Encodable, VarInt};
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };

    #[test]
    fn tree_branch_shape_matches_monero_tree_algorithm() {
        let hashes = [
            B256::repeat_byte(1),
            B256::repeat_byte(2),
            B256::repeat_byte(3),
            B256::repeat_byte(4),
            B256::repeat_byte(5),
        ];
        let root = monero_tree_hash_for_test(&hashes);
        for index in 0..hashes.len() {
            let branch = monero_tree_branch_for_test(&hashes, index);
            let (path, depth) = monero_tree_path(hashes.len(), index).unwrap();
            assert_eq!(depth, branch.len());
            assert_eq!(
                verify_monero_tree_branch(hashes[index], &branch, path),
                root
            );
        }
    }

    #[test]
    fn pinned_block_hash_rejects_bad_anchor() {
        let header = synthetic_header_bytes(B256::repeat_byte(0x11));
        let root = B256::repeat_byte(0x22);
        let hash = monero_block_hash(&header, root, 1).unwrap();
        assert_ne!(hash, B256::repeat_byte(0x33));
    }

    #[test]
    fn verifies_real_stage_a_reserve_fixture() {
        let witness = stage_b_fixture();
        let verified = verify_xmr_reserve_witness(&witness).unwrap();
        assert_eq!(verified.xmr_atomic, 154_190_240_000);
        assert_eq!(
            verified.key_images,
            vec![hex_b256(
                "2b2d8f7fc5d7ac78d2c2a9e0c5f726c75119dbdcabd26dc331d2a026dc781c2b"
            )]
        );
        assert_eq!(
            verified.output_block_hashes,
            vec![hex_b256(
                "6026747c117e6707749d07fa2de4362b41a9863eed5527494ea03ea9917e3c50"
            )]
        );
    }

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

    fn context<'a>(
        policy: &MoneroReservePolicyV1,
        commitment: &'a str,
    ) -> MoneroReserveVerifyContextV1<'a> {
        MoneroReserveVerifyContextV1 {
            pftl_genesis_hash: "11".repeat(48).leak(),
            nav_asset_id: "22".repeat(48).leak(),
            proof_profile_id: "33".repeat(48).leak(),
            valuation_policy_hash: "44".repeat(32).leak(),
            source_manifest_hash: "55".repeat(48).leak(),
            source_id: "monero-primary",
            source_domain: policy.source_domain.clone().leak(),
            asset_or_position_id: policy.position_id.clone().leak(),
            reserve_owner_commitment: monero_reserve_owner_commitment(
                policy.address_spend_public_key,
                policy.address_view_public_key,
            )
            .leak(),
            quantity_verifier_commitment: policy.commitment().unwrap().leak(),
            observation_epoch: 9,
            observation_not_before: 500,
            observation_not_after: 500,
            observed_at_pftl_height: 500,
            expected_evidence_commitment: commitment,
        }
    }

    fn sign_monero_message(secret: Scalar, message_hash: [u8; 32]) -> (B256, XmrSignature) {
        let public = (ED25519_BASEPOINT_POINT * secret).compress().to_bytes();
        let nonce = Scalar::from(123_456u64);
        let commitment = (ED25519_BASEPOINT_POINT * nonce).compress().to_bytes();
        let mut challenge = Vec::new();
        challenge.extend_from_slice(&message_hash);
        challenge.extend_from_slice(&public);
        challenge.extend_from_slice(&commitment);
        let c = hash_to_scalar(&challenge);
        let r = nonce - (c * secret);
        (
            B256::from(public),
            XmrSignature {
                c: B256::from(c.to_bytes()),
                r: B256::from(r.to_bytes()),
            },
        )
    }

    fn zero_fixture() -> MoneroReserveProofV1 {
        let spend_secret = Scalar::from(42u64);
        let spend = B256::from(
            (ED25519_BASEPOINT_POINT * spend_secret)
                .compress()
                .to_bytes(),
        );
        let view = B256::from(
            (ED25519_BASEPOINT_POINT * Scalar::from(43u64))
                .compress()
                .to_bytes(),
        );
        let (committee, keys) = committee();
        let policy = MoneroReservePolicyV1 {
            source_domain: "monero:mainnet".to_string(),
            position_id: "monero-reserve:a666-v1".to_string(),
            address_spend_public_key: spend,
            address_view_public_key: view,
            checkpoint_committee_root: committee.root().unwrap(),
            allow_pinned_output_blocks: false,
            max_outputs: 64,
            max_header_links_per_output: 256,
        };
        let mut reserve = XmrReserveWitness {
            address_spend_public_key: spend,
            address_view_public_key: view,
            message: monero_reserve_challenge_v1(&policy, &context(&policy, "00")).unwrap(),
            entries: Vec::new(),
            subaddr_spendkeys: Vec::new(),
        };
        let prefix = reserve_prefix_hash(&reserve);
        let (public_key, signature) = sign_monero_message(spend_secret, prefix);
        reserve.subaddr_spendkeys.push(XmrSubaddrSpendKeyProof {
            archive_marker: 0,
            public_key,
            signature,
        });
        let head_hash = B256::repeat_byte(0x77);
        let status = monero_status_commitment(head_hash, &[], &[]).unwrap();
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: MONERO_CHECKPOINT_KIND_V1.to_string(),
            source_domain: policy.source_domain.clone(),
            source_height: 3_500_000,
            source_timestamp_ms: 1_785_000_000_000,
            source_block_hash: head_hash,
            source_state_commitment: status,
            observed_source_head: 3_500_012,
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
        MoneroReserveProofV1 {
            policy,
            checkpoint_certificate: certificate,
            reserve,
            key_image_statuses: Vec::new(),
        }
    }

    #[test]
    fn verifies_fresh_context_bound_zero_reserve_without_attested_quantity() {
        let proof = zero_fixture();
        let commitment = proof.evidence_commitment().unwrap();
        let verified =
            verify_monero_reserve_proof_v1(&proof, &context(&proof.policy, &commitment)).unwrap();
        assert_eq!(verified.xmr_atomic, 0);
        assert!(verified.key_images.is_empty());
    }

    #[test]
    fn rejects_non_mainnet_policy_domain() {
        let mut proof = zero_fixture();
        proof.policy.source_domain = "monero:stagenet".to_string();
        assert_eq!(
            proof.policy.validate(),
            Err(XmrReserveLegError::UnsupportedShape)
        );
    }

    #[test]
    fn registered_guest_dispatch_executes_monero_zero_proof_as_cryptographic() {
        use crate::{
            execute_reserve_proof, FreshnessPolicyV1, LiabilityTreatmentV1, ReserveProofContextV1,
            ReserveProofWitnessV1, SourceEvidenceV1, SourceManifestEntryV1, SourceManifestV1,
            SourceObservationV1, TrustClassV1, MANIFEST_SCHEMA_V1, WITNESS_SCHEMA_V1,
        };

        let mut proof = zero_fixture();
        let manifest = SourceManifestV1 {
            schema: MANIFEST_SCHEMA_V1.to_string(),
            sources: vec![SourceManifestEntryV1 {
                source_id: "monero-primary".to_string(),
                adapter_kind: MONERO_RESERVE_ADAPTER_KIND_V1.to_string(),
                source_domain: proof.policy.source_domain.clone(),
                asset_or_position_id: proof.policy.position_id.clone(),
                reserve_owner_commitment: monero_reserve_owner_commitment(
                    proof.policy.address_spend_public_key,
                    proof.policy.address_view_public_key,
                ),
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
            observation_epoch: 9,
            observation_not_before: 500,
            observation_not_after: 500,
        };
        let verify_context = MoneroReserveVerifyContextV1 {
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
            observation_epoch: reserve_context.observation_epoch,
            observation_not_before: reserve_context.observation_not_before,
            observation_not_after: reserve_context.observation_not_after,
            observed_at_pftl_height: 500,
            expected_evidence_commitment: "00",
        };
        proof.reserve.message =
            monero_reserve_challenge_v1(&proof.policy, &verify_context).unwrap();
        proof.reserve.subaddr_spendkeys.clear();
        let prefix = reserve_prefix_hash(&proof.reserve);
        let (public_key, signature) = sign_monero_message(Scalar::from(42u64), prefix);
        proof
            .reserve
            .subaddr_spendkeys
            .push(XmrSubaddrSpendKeyProof {
                archive_marker: 0,
                public_key,
                signature,
            });
        let evidence_commitment = proof.evidence_commitment().unwrap();
        let witness = ReserveProofWitnessV1 {
            schema: WITNESS_SCHEMA_V1.to_string(),
            context: reserve_context,
            manifest,
            observations: vec![SourceObservationV1 {
                source_id: "monero-primary".to_string(),
                observed_at_block: 500,
                gross_assets: 0,
                total_liabilities: 0,
                quantity_evidence: SourceEvidenceV1::MoneroReserve {
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
        assert_eq!(public.quantity_trust_counts.cryptographic, 1);
        assert_eq!(public.gross_assets, 0);
    }

    #[test]
    fn wrapper_rejects_replay_spent_status_head_and_context_substitution() {
        let proof = zero_fixture();
        let commitment = proof.evidence_commitment().unwrap();

        let mut replay = context(&proof.policy, &commitment);
        replay.observation_epoch += 1;
        assert_eq!(
            verify_monero_reserve_proof_v1(&proof, &replay),
            Err(XmrReserveLegError::BadAddressBinding)
        );

        let mut bad_head = proof.clone();
        bad_head
            .checkpoint_certificate
            .checkpoint
            .source_state_commitment = B256::repeat_byte(0x99);
        let commitment = bad_head.evidence_commitment().unwrap();
        assert_eq!(
            verify_monero_reserve_proof_v1(&bad_head, &context(&bad_head.policy, &commitment)),
            Err(XmrReserveLegError::BadBlockAnchor)
        );

        let status = MoneroKeyImageStatusV1 {
            key_image: B256::repeat_byte(1),
            spent: true,
        };
        assert_ne!(
            monero_status_commitment(B256::repeat_byte(2), &[], &[status]).unwrap(),
            B256::ZERO
        );
    }

    #[test]
    fn key_image_statuses_are_complete_sorted_and_unspent() {
        let first = B256::repeat_byte(1);
        let second = B256::repeat_byte(2);
        let statuses = [
            MoneroKeyImageStatusV1 {
                key_image: first,
                spent: false,
            },
            MoneroKeyImageStatusV1 {
                key_image: second,
                spent: false,
            },
        ];
        assert_eq!(
            validate_key_image_statuses(&statuses, &[second, first]),
            Ok(())
        );

        let mut unsorted = statuses.clone();
        unsorted.swap(0, 1);
        assert_eq!(
            validate_key_image_statuses(&unsorted, &[first, second]),
            Err(XmrReserveLegError::BadKeyImageStatusSet)
        );

        let mut spent = statuses;
        spent[1].spent = true;
        assert_eq!(
            validate_key_image_statuses(&spent, &[first, second]),
            Err(XmrReserveLegError::SpentKeyImage)
        );
    }

    #[test]
    fn tampered_spend_signature_fails_closed() {
        let mut witness = stage_b_fixture();
        let mut c = witness.subaddr_spendkeys[0].signature.c.0;
        c[0] ^= 1;
        witness.subaddr_spendkeys[0].signature.c = B256::from(c);
        assert_eq!(
            verify_xmr_reserve_witness(&witness),
            Err(XmrReserveLegError::BadSubaddressSpendKeySignature)
        );
    }

    #[test]
    fn tampered_transaction_bytes_fail_closed() {
        let mut witness = stage_b_fixture();
        let last = witness.entries[0].transaction_bytes.len() - 1;
        witness.entries[0].transaction_bytes[last] ^= 1;
        assert!(matches!(
            verify_xmr_reserve_witness(&witness),
            Err(XmrReserveLegError::BadTransaction | XmrReserveLegError::BadTransactionHash)
        ));
    }

    #[test]
    fn tampered_tx_inclusion_fails_closed() {
        let mut witness = stage_b_fixture();
        witness.entries[0].tx_tree.branch[0] = B256::repeat_byte(0x42);
        assert_eq!(
            verify_xmr_reserve_witness(&witness),
            Err(XmrReserveLegError::BadTxTreeProof)
        );
    }

    #[test]
    fn tampered_block_anchor_fails_closed() {
        let mut witness = stage_b_fixture();
        witness.entries[0].block_anchor = XmrBlockAnchor::PinnedOutputBlock {
            pinned_output_block_hash: B256::repeat_byte(0x55),
        };
        assert_eq!(
            verify_xmr_reserve_witness(&witness),
            Err(XmrReserveLegError::BadBlockAnchor)
        );
    }

    fn monero_tree_hash_for_test(hashes: &[B256]) -> B256 {
        match hashes.len() {
            0 => panic!("empty tree"),
            1 => hashes[0],
            2 => keccak_pair(hashes[0], hashes[1]),
            count => {
                let mut cnt = tree_hash_cnt(count);
                let mut work = vec![B256::ZERO; cnt];
                let initial = 2 * cnt - count;
                work[..initial].copy_from_slice(&hashes[..initial]);
                let mut i = initial;
                let mut j = initial;
                while j < cnt {
                    work[j] = keccak_pair(hashes[i], hashes[i + 1]);
                    i += 2;
                    j += 1;
                }
                while cnt > 2 {
                    cnt >>= 1;
                    for j in 0..cnt {
                        work[j] = keccak_pair(work[2 * j], work[2 * j + 1]);
                    }
                }
                keccak_pair(work[0], work[1])
            }
        }
    }

    fn monero_tree_branch_for_test(hashes: &[B256], mut idx: usize) -> Vec<B256> {
        let count = hashes.len();
        let mut siblings = Vec::new();
        if count == 1 {
            return siblings;
        }
        if count == 2 {
            siblings.push(hashes[idx ^ 1]);
            return siblings;
        }

        let mut cnt = tree_hash_cnt(count);
        let mut work = vec![B256::ZERO; cnt];
        let initial = 2 * cnt - count;
        work[..initial].copy_from_slice(&hashes[..initial]);
        let mut i = initial;
        let mut j = initial;
        while j < cnt {
            if idx == i || idx == i + 1 {
                siblings.push(hashes[if idx == i { i + 1 } else { i }]);
                idx = j;
            }
            work[j] = keccak_pair(hashes[i], hashes[i + 1]);
            i += 2;
            j += 1;
        }
        while cnt > 2 {
            cnt >>= 1;
            for i in 0..cnt {
                if idx == 2 * i || idx == 2 * i + 1 {
                    siblings.push(work[if idx == 2 * i { 2 * i + 1 } else { 2 * i }]);
                    idx = i;
                }
                work[i] = keccak_pair(work[2 * i], work[2 * i + 1]);
            }
        }
        if idx == 0 || idx == 1 {
            siblings.push(work[if idx == 0 { 1 } else { 0 }]);
        }
        siblings
    }

    fn synthetic_header_bytes(prev_hash: B256) -> Vec<u8> {
        let header = BlockHeader {
            major_version: VarInt(16),
            minor_version: VarInt(16),
            timestamp: VarInt(1_776_000_000),
            prev_id: monero::Hash(prev_hash.0),
            nonce: 7,
        };
        let mut bytes = Vec::new();
        header.consensus_encode(&mut bytes).unwrap();
        bytes
    }

    fn stage_b_fixture() -> XmrReserveWitness {
        serde_json::from_str(include_str!(
            "../../../../../docs/fixtures/open-reserve-proof/xmr_reserve_stage_b_witness.json"
        ))
        .unwrap()
    }

    fn hex_b256(value: &str) -> B256 {
        B256::from_slice(&hex::decode(value).unwrap())
    }
}
