use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Bytes, B256};
use alloy_rlp::Header;
use alloy_trie::{proof::verify_proof, HashBuilder, Nibbles};
use serde::{Deserialize, Serialize};

use crate::{decode_fixed_hex, encode_hex, ConformanceError, ARC_TESTNET_CHAIN_ID};

pub const MAX_RECEIPTS_PER_BLOCK: usize = 16_384;
pub const MAX_RECEIPT_PROOF_NODES: usize = 64;
pub const MAX_RECEIPT_PROOF_NODE_BYTES: usize = 16_384;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcReceipt {
    pub transaction_hash: String,
    pub transaction_index: String,
    #[serde(rename = "type")]
    pub receipt_type: String,
    pub status: String,
    pub cumulative_gas_used: String,
    pub logs_bloom: String,
    pub logs: Vec<RpcLog>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RpcLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptProofFixture {
    pub schema: String,
    pub chain_id: u64,
    pub block: ReceiptBlockFixture,
    pub receipts: Vec<ReceiptInclusionFixture>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptBlockFixture {
    pub number: u64,
    pub hash: String,
    pub receipts_root: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptInclusionFixture {
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub receipt_type: u8,
    pub encoded_receipt: String,
    pub proof_nodes: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReceiptConformanceError {
    #[error("unsupported receipt fixture schema: {0}")]
    Schema(String),
    #[error("wrong Arc chain id: {0}")]
    ChainId(u64),
    #[error("receipt fixture exceeds configured bounds")]
    Bounds,
    #[error("invalid receipt field: {0}")]
    InvalidField(&'static str),
    #[error("duplicate transaction index: {0}")]
    DuplicateIndex(u64),
    #[error("computed receipts root does not match block header")]
    RootMismatch,
    #[error("receipt inclusion proof failed")]
    InvalidProof,
    #[error("fixture does not cover both a legacy and EIP-2718 receipt")]
    MissingReceiptTypeCoverage,
}

pub fn build_receipt_fixture(
    chain_id: u64,
    block: ReceiptBlockFixture,
    rpc_receipts: &[RpcReceipt],
) -> Result<ReceiptProofFixture, ReceiptConformanceError> {
    if rpc_receipts.is_empty() || rpc_receipts.len() > MAX_RECEIPTS_PER_BLOCK {
        return Err(ReceiptConformanceError::Bounds);
    }
    let mut leaves = BTreeMap::new();
    let mut metadata = BTreeMap::new();
    let mut target_indices = BTreeSet::new();

    for receipt in rpc_receipts {
        let index = parse_quantity(&receipt.transaction_index, "transactionIndex")?;
        let receipt_type_u64 = parse_quantity(&receipt.receipt_type, "type")?;
        let receipt_type = u8::try_from(receipt_type_u64)
            .map_err(|_| ReceiptConformanceError::InvalidField("type"))?;
        let encoded = encode_receipt(receipt, receipt_type)?;
        let key = receipt_path(index);
        if leaves.insert(key, encoded).is_some() {
            return Err(ReceiptConformanceError::DuplicateIndex(index));
        }
        metadata.insert(index, (receipt.transaction_hash.clone(), receipt_type));
        if receipt_type == 0 && !target_indices.iter().any(|i| metadata[i].1 == 0) {
            target_indices.insert(index);
        }
        if receipt_type != 0 && !target_indices.iter().any(|i| metadata[i].1 != 0) {
            target_indices.insert(index);
        }
    }
    if !target_indices.iter().any(|i| metadata[i].1 == 0)
        || !target_indices.iter().any(|i| metadata[i].1 != 0)
    {
        return Err(ReceiptConformanceError::MissingReceiptTypeCoverage);
    }

    let targets = target_indices.iter().copied().map(receipt_path);
    let mut builder = HashBuilder::default()
        .with_proof_retainer(alloy_trie::proof::ProofRetainer::from_iter(targets));
    for (key, value) in &leaves {
        builder.add_leaf(*key, value);
    }
    let computed_root = builder.root();
    let expected_root = B256::from(
        decode_fixed_hex::<32>(&block.receipts_root, "block.receipts_root")
            .map_err(map_hex_error)?,
    );
    if computed_root != expected_root {
        return Err(ReceiptConformanceError::RootMismatch);
    }
    let proof_nodes = builder.take_proof_nodes();

    let mut receipts = Vec::new();
    for index in target_indices {
        let path = receipt_path(index);
        let encoded = leaves
            .get(&path)
            .ok_or(ReceiptConformanceError::InvalidField("transactionIndex"))?;
        let (transaction_hash, receipt_type) = metadata
            .get(&index)
            .ok_or(ReceiptConformanceError::InvalidField("transactionIndex"))?;
        receipts.push(ReceiptInclusionFixture {
            transaction_hash: transaction_hash.clone(),
            transaction_index: index,
            receipt_type: *receipt_type,
            encoded_receipt: encode_hex(encoded),
            proof_nodes: proof_nodes
                .matching_nodes_sorted(&path)
                .into_iter()
                .map(|(_, node)| encode_hex(node))
                .collect(),
        });
    }

    let fixture = ReceiptProofFixture {
        schema: "arc-receipt-proof-fixture-v1".to_owned(),
        chain_id,
        block,
        receipts,
    };
    verify_receipt_fixture(&fixture)?;
    Ok(fixture)
}

/// Reconstruct the complete receipts trie and retain the inclusion proof for
/// one exact transaction index. Unlike `build_receipt_fixture`, this is the
/// production witness-builder path and does not require mixed receipt types in
/// the selected block.
pub fn build_receipt_inclusion(
    chain_id: u64,
    block: &ReceiptBlockFixture,
    rpc_receipts: &[RpcReceipt],
    target_transaction_index: u64,
) -> Result<ReceiptInclusionFixture, ReceiptConformanceError> {
    if chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(ReceiptConformanceError::ChainId(chain_id));
    }
    if rpc_receipts.is_empty() || rpc_receipts.len() > MAX_RECEIPTS_PER_BLOCK {
        return Err(ReceiptConformanceError::Bounds);
    }

    let mut leaves = BTreeMap::new();
    let mut metadata = BTreeMap::new();
    for receipt in rpc_receipts {
        let index = parse_quantity(&receipt.transaction_index, "transactionIndex")?;
        let receipt_type_u64 = parse_quantity(&receipt.receipt_type, "type")?;
        let receipt_type = u8::try_from(receipt_type_u64)
            .map_err(|_| ReceiptConformanceError::InvalidField("type"))?;
        let encoded = encode_receipt(receipt, receipt_type)?;
        if leaves.insert(receipt_path(index), encoded).is_some() {
            return Err(ReceiptConformanceError::DuplicateIndex(index));
        }
        metadata.insert(index, (receipt.transaction_hash.clone(), receipt_type));
    }

    let target_path = receipt_path(target_transaction_index);
    let encoded = leaves
        .get(&target_path)
        .ok_or(ReceiptConformanceError::InvalidField(
            "targetTransactionIndex",
        ))?;
    let (transaction_hash, receipt_type) =
        metadata
            .get(&target_transaction_index)
            .ok_or(ReceiptConformanceError::InvalidField(
                "targetTransactionIndex",
            ))?;
    let mut builder = HashBuilder::default().with_proof_retainer(
        alloy_trie::proof::ProofRetainer::from_iter(std::iter::once(target_path)),
    );
    for (key, value) in &leaves {
        builder.add_leaf(*key, value);
    }
    let computed_root = builder.root();
    let expected_root = B256::from(
        decode_fixed_hex::<32>(&block.receipts_root, "block.receipts_root")
            .map_err(map_hex_error)?,
    );
    if computed_root != expected_root {
        return Err(ReceiptConformanceError::RootMismatch);
    }
    let proof_nodes = builder
        .take_proof_nodes()
        .matching_nodes_sorted(&target_path)
        .into_iter()
        .map(|(_, node)| encode_hex(node))
        .collect::<Vec<_>>();
    if proof_nodes.is_empty() || proof_nodes.len() > MAX_RECEIPT_PROOF_NODES {
        return Err(ReceiptConformanceError::Bounds);
    }

    let proof = ReceiptInclusionFixture {
        transaction_hash: transaction_hash.clone(),
        transaction_index: target_transaction_index,
        receipt_type: *receipt_type,
        encoded_receipt: encode_hex(encoded),
        proof_nodes,
    };
    let nodes = proof
        .proof_nodes
        .iter()
        .map(|node| decode_variable_hex(node, "proof_nodes").map(Bytes::from))
        .collect::<Result<Vec<_>, _>>()?;
    verify_proof(
        expected_root,
        target_path,
        Some(encoded.clone()),
        nodes.iter(),
    )
    .map_err(|_| ReceiptConformanceError::InvalidProof)?;
    Ok(proof)
}

pub fn verify_receipt_fixture(
    fixture: &ReceiptProofFixture,
) -> Result<(), ReceiptConformanceError> {
    if fixture.schema != "arc-receipt-proof-fixture-v1" {
        return Err(ReceiptConformanceError::Schema(fixture.schema.clone()));
    }
    if fixture.chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(ReceiptConformanceError::ChainId(fixture.chain_id));
    }
    if fixture.receipts.len() < 2 || fixture.receipts.len() > MAX_RECEIPTS_PER_BLOCK {
        return Err(ReceiptConformanceError::Bounds);
    }
    if !fixture
        .receipts
        .iter()
        .any(|receipt| receipt.receipt_type == 0)
        || !fixture
            .receipts
            .iter()
            .any(|receipt| receipt.receipt_type != 0)
    {
        return Err(ReceiptConformanceError::MissingReceiptTypeCoverage);
    }
    let root = B256::from(
        decode_fixed_hex::<32>(&fixture.block.receipts_root, "block.receipts_root")
            .map_err(map_hex_error)?,
    );
    let mut indices = BTreeSet::new();
    for receipt in &fixture.receipts {
        if !indices.insert(receipt.transaction_index) {
            return Err(ReceiptConformanceError::DuplicateIndex(
                receipt.transaction_index,
            ));
        }
        if receipt.proof_nodes.is_empty() || receipt.proof_nodes.len() > MAX_RECEIPT_PROOF_NODES {
            return Err(ReceiptConformanceError::Bounds);
        }
        let value = decode_variable_hex(&receipt.encoded_receipt, "encoded_receipt")?;
        if (receipt.receipt_type == 0 && value.first().is_some_and(|byte| *byte < 0xc0))
            || (receipt.receipt_type != 0 && value.first().copied() != Some(receipt.receipt_type))
        {
            return Err(ReceiptConformanceError::InvalidField("encoded_receipt"));
        }
        let nodes: Vec<Bytes> = receipt
            .proof_nodes
            .iter()
            .map(|node| decode_variable_hex(node, "proof_nodes").map(Bytes::from))
            .collect::<Result<_, _>>()?;
        if nodes
            .iter()
            .any(|node| node.len() > MAX_RECEIPT_PROOF_NODE_BYTES)
        {
            return Err(ReceiptConformanceError::Bounds);
        }
        verify_proof(
            root,
            receipt_path(receipt.transaction_index),
            Some(value),
            nodes.iter(),
        )
        .map_err(|_| ReceiptConformanceError::InvalidProof)?;
    }
    Ok(())
}

pub fn encode_receipt(
    receipt: &RpcReceipt,
    receipt_type: u8,
) -> Result<Vec<u8>, ReceiptConformanceError> {
    let status = parse_quantity(&receipt.status, "status")?;
    if status > 1 {
        return Err(ReceiptConformanceError::InvalidField("status"));
    }
    let cumulative_gas = parse_quantity(&receipt.cumulative_gas_used, "cumulativeGasUsed")?;
    let bloom = decode_variable_hex(&receipt.logs_bloom, "logsBloom")?;
    if bloom.len() != 256 {
        return Err(ReceiptConformanceError::InvalidField("logsBloom"));
    }

    let status_rlp = alloy_rlp::encode(status);
    let cumulative_gas_rlp = alloy_rlp::encode(cumulative_gas);
    let bloom_rlp = alloy_rlp::encode(bloom.as_slice());
    let mut encoded_logs = Vec::with_capacity(receipt.logs.len());
    for log in &receipt.logs {
        let address = decode_variable_hex(&log.address, "log.address")?;
        if address.len() != 20 {
            return Err(ReceiptConformanceError::InvalidField("log.address"));
        }
        let address_rlp = alloy_rlp::encode(address.as_slice());
        let topics = log
            .topics
            .iter()
            .map(|topic| {
                let topic = decode_variable_hex(topic, "log.topics")?;
                if topic.len() != 32 {
                    return Err(ReceiptConformanceError::InvalidField("log.topics"));
                }
                Ok(alloy_rlp::encode(topic.as_slice()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let topics_rlp = encode_list(&topics);
        let data = decode_variable_hex(&log.data, "log.data")?;
        let data_rlp = alloy_rlp::encode(data.as_slice());
        encoded_logs.push(encode_list(&[address_rlp, topics_rlp, data_rlp]));
    }
    let logs_rlp = encode_list(&encoded_logs);
    let payload = encode_list(&[status_rlp, cumulative_gas_rlp, bloom_rlp, logs_rlp]);
    if receipt_type == 0 {
        Ok(payload)
    } else {
        let mut typed = Vec::with_capacity(payload.len() + 1);
        typed.push(receipt_type);
        typed.extend_from_slice(&payload);
        Ok(typed)
    }
}

fn encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload_length = items.iter().map(Vec::len).sum();
    let header = Header {
        list: true,
        payload_length,
    };
    let mut out = Vec::with_capacity(header.length_with_payload());
    header.encode(&mut out);
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

fn receipt_path(index: u64) -> Nibbles {
    Nibbles::unpack(alloy_rlp::encode(index))
}

fn parse_quantity(value: &str, field: &'static str) -> Result<u64, ReceiptConformanceError> {
    let raw = value
        .strip_prefix("0x")
        .ok_or(ReceiptConformanceError::InvalidField(field))?;
    u64::from_str_radix(raw, 16).map_err(|_| ReceiptConformanceError::InvalidField(field))
}

fn decode_variable_hex(
    value: &str,
    field: &'static str,
) -> Result<Vec<u8>, ReceiptConformanceError> {
    let raw = value
        .strip_prefix("0x")
        .ok_or(ReceiptConformanceError::InvalidField(field))?;
    hex::decode(raw).map_err(|_| ReceiptConformanceError::InvalidField(field))
}

fn map_hex_error(_: ConformanceError) -> ReceiptConformanceError {
    ReceiptConformanceError::InvalidField("block.receipts_root")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../fixtures/arc-receipts.json");

    #[test]
    fn receipt_inclusion_conformance() {
        let fixture: ReceiptProofFixture = serde_json::from_str(FIXTURE).unwrap();
        verify_receipt_fixture(&fixture).expect("legacy and typed Arc receipts must verify");

        for target in 0..fixture.receipts.len() {
            let mut mutated = fixture.clone();
            let encoded = &mut mutated.receipts[target].encoded_receipt;
            let last = encoded.pop().expect("encoded receipt is non-empty");
            encoded.push(if last == '0' { '1' } else { '0' });
            assert_eq!(
                verify_receipt_fixture(&mutated),
                Err(ReceiptConformanceError::InvalidProof)
            );
        }
    }
}
