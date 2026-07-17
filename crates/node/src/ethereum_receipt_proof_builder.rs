use std::array;
use std::io;
use std::path::PathBuf;

use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes};
use postfiat_storage::{atomic_write, NodeStore};
use postfiat_types::{EthereumReceiptProofV1, ETHEREUM_RECEIPT_PROOF_MAX_RECEIPT_BYTES};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ethereum_checkpoint_signing::{
    ethereum_rpc_call, ethereum_rpc_call_with_limit, EthereumRpcEndpoint,
};

const ETHEREUM_RECEIPT_PROOF_ARTIFACT_SCHEMA_V1: &str =
    "postfiat-ethereum-receipt-proof-artifact-v1";
const MAX_BLOCK_RECEIPTS: usize = 8_192;
const MAX_LOGS_PER_RECEIPT: usize = 4_096;
const MAX_BLOCK_RECEIPTS_RPC_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumReceiptProofBuildOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub ethereum_rpc: String,
    pub transaction_hash: String,
    pub proof_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumReceiptProofArtifactV1 {
    pub schema: String,
    pub route_id: String,
    pub ethereum_chain_id: u64,
    pub transaction_hash: String,
    pub block_number: u64,
    pub block_hash: String,
    pub receipts_root: String,
    pub proof: EthereumReceiptProofV1,
}

pub fn build_ethereum_receipt_proof(
    options: EthereumReceiptProofBuildOptions,
) -> io::Result<EthereumReceiptProofArtifactV1> {
    let store = NodeStore::new(&options.data_dir);
    let ledger = store.read_ledger()?;
    let route = ledger
        .pftl_uniswap_route(&options.route_id)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "receipt-proof route is not live")
        })?;
    route.validate().map_err(invalid_data)?;
    if route.route_trust_class != postfiat_bridge::ROUTE_TRUST_CLASS_BFT_CHECKPOINT
        || route.ethereum_verification_policy.is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "receipt-proof route is not governed by the BFT_CHECKPOINT verification policy",
        ));
    }

    let transaction_hash = canonical_hash("transaction hash", &options.transaction_hash)?;
    let endpoint = EthereumRpcEndpoint::parse(&options.ethereum_rpc)?;
    let chain_id = rpc_quantity(&endpoint, "eth_chainId", serde_json::json!([]))?;
    if chain_id != route.ethereum_chain_id {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Ethereum RPC chain ID does not match the governed route",
        ));
    }

    let target_receipt = ethereum_rpc_call(
        &endpoint,
        "eth_getTransactionReceipt",
        serde_json::json!([transaction_hash]),
    )?;
    if target_receipt.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Ethereum transaction receipt is not available",
        ));
    }
    let block_hash = canonical_hash(
        "receipt block hash",
        required_string(&target_receipt, "blockHash")?,
    )?;
    let block_number = quantity_field(&target_receipt, "blockNumber")?;
    let target_index = quantity_field(&target_receipt, "transactionIndex")?;

    let block = ethereum_rpc_call(
        &endpoint,
        "eth_getBlockByHash",
        serde_json::json!([block_hash, false]),
    )?;
    if block.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Ethereum receipt block is not available",
        ));
    }
    if canonical_hash("block hash", required_string(&block, "hash")?)? != block_hash
        || quantity_field(&block, "number")? != block_number
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum block response does not match the target receipt",
        ));
    }
    let receipts_root = exact_hex::<32>(
        "block receipts root",
        required_string(&block, "receiptsRoot")?,
    )?;
    let transactions = block
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum block has no transaction array",
            )
        })?;
    if transactions.is_empty() || transactions.len() > MAX_BLOCK_RECEIPTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum block transaction count is empty or exceeds the bounded proof-builder limit",
        ));
    }

    let block_receipts = ethereum_rpc_call_with_limit(
        &endpoint,
        "eth_getBlockReceipts",
        serde_json::json!([format!("0x{block_number:x}")]),
        MAX_BLOCK_RECEIPTS_RPC_BYTES,
    )?;
    let receipts = block_receipts.as_array().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "eth_getBlockReceipts did not return an array",
        )
    })?;
    if receipts.len() != transactions.len() || receipts.len() > MAX_BLOCK_RECEIPTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum block receipt count does not match its transaction count",
        ));
    }

    let mut ordered = vec![None; receipts.len()];
    for receipt in receipts {
        let index_u64 = quantity_field(receipt, "transactionIndex")?;
        let index = usize::try_from(index_u64).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum transaction index exceeds this platform",
            )
        })?;
        if index >= ordered.len() || ordered[index].is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum block receipts contain a duplicate or out-of-range transaction index",
            ));
        }
        let transaction_hash = canonical_hash(
            "receipt transaction hash",
            required_string(receipt, "transactionHash")?,
        )?;
        let block_transaction_hash = canonical_hash(
            "block transaction hash",
            transactions[index].as_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Ethereum block transaction entry is not a hash",
                )
            })?,
        )?;
        if transaction_hash != block_transaction_hash
            || canonical_hash("receipt block hash", required_string(receipt, "blockHash")?)?
                != block_hash
            || quantity_field(receipt, "blockNumber")? != block_number
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum receipt does not match its canonical block transaction",
            ));
        }
        ordered[index] = Some((transaction_hash, encode_receipt(receipt)?));
    }
    let ordered = ordered
        .into_iter()
        .map(|receipt| {
            receipt.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Ethereum block receipt indexes are not contiguous",
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let target_index_usize = usize::try_from(target_index).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "target transaction index exceeds this platform",
        )
    })?;
    let Some((ordered_target_hash, target_receipt_rlp)) = ordered.get(target_index_usize) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "target transaction index is outside the receipt set",
        ));
    };
    if ordered_target_hash != &transaction_hash
        || encode_receipt(&target_receipt)? != *target_receipt_rlp
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "target receipt does not byte-match the canonical block receipt set",
        ));
    }

    let entries = ordered
        .iter()
        .enumerate()
        .map(|(index, (_, receipt))| {
            Ok(TrieEntry {
                key: bytes_to_nibbles(&rlp_u64(u64::try_from(index).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "receipt index exceeds u64")
                })?)),
                value: receipt.clone(),
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let entry_refs = entries.iter().collect::<Vec<_>>();
    let trie = build_trie(&entry_refs, 0)?;
    let computed_root = postfiat_bridge::ethereum_keccak256(&trie.encoded);
    if computed_root != receipts_root {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "locally reconstructed Ethereum receipt trie does not match the block receiptsRoot",
        ));
    }
    let target_key = bytes_to_nibbles(&rlp_u64(target_index));
    let mut proof_nodes_rlp = Vec::new();
    collect_proof(&trie, &target_key, 0, &mut proof_nodes_rlp)?;
    let proof = EthereumReceiptProofV1 {
        transaction_index: target_index,
        receipt_rlp: target_receipt_rlp.clone(),
        proof_nodes_rlp,
    };
    proof.validate_bounds().map_err(invalid_data)?;

    let artifact = EthereumReceiptProofArtifactV1 {
        schema: ETHEREUM_RECEIPT_PROOF_ARTIFACT_SCHEMA_V1.to_string(),
        route_id: route.route_id.clone(),
        ethereum_chain_id: chain_id,
        transaction_hash,
        block_number,
        block_hash,
        receipts_root: format!("0x{}", bytes_to_hex(&receipts_root)),
        proof,
    };
    write_json(&options.proof_file, &artifact)?;
    Ok(artifact)
}

#[derive(Debug, Clone)]
struct TrieEntry {
    key: Vec<u8>,
    value: Vec<u8>,
}

#[derive(Debug)]
struct TrieNode {
    encoded: Vec<u8>,
    kind: TrieNodeKind,
}

#[derive(Debug)]
enum TrieNodeKind {
    Leaf {
        path: Vec<u8>,
        value: Vec<u8>,
    },
    Extension {
        path: Vec<u8>,
        child: Box<TrieNode>,
    },
    Branch {
        children: [Option<Box<TrieNode>>; 16],
        value: Option<Vec<u8>>,
    },
}

fn build_trie(entries: &[&TrieEntry], depth: usize) -> io::Result<TrieNode> {
    if entries.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot build an empty Ethereum receipt trie",
        ));
    }
    if entries.len() == 1 {
        let entry = entries[0];
        if depth > entry.key.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum receipt trie depth exceeds its key",
            ));
        }
        let path = entry.key[depth..].to_vec();
        let encoded = rlp_list(&[
            rlp_bytes(&compact_path(&path, true)?),
            rlp_bytes(&entry.value),
        ]);
        return Ok(TrieNode {
            encoded,
            kind: TrieNodeKind::Leaf {
                path,
                value: entry.value.clone(),
            },
        });
    }

    let common = common_prefix_len(entries, depth)?;
    if common > 0 {
        let path = entries[0].key[depth..depth + common].to_vec();
        let child = Box::new(build_trie(entries, depth + common)?);
        let encoded = rlp_list(&[
            rlp_bytes(&compact_path(&path, false)?),
            node_reference(&child),
        ]);
        return Ok(TrieNode {
            encoded,
            kind: TrieNodeKind::Extension { path, child },
        });
    }

    let mut groups: [Vec<&TrieEntry>; 16] = array::from_fn(|_| Vec::new());
    let mut value = None;
    for entry in entries {
        if depth == entry.key.len() {
            if value.replace(entry.value.clone()).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "duplicate Ethereum receipt trie key",
                ));
            }
        } else {
            let nibble = usize::from(entry.key[depth]);
            if nibble >= groups.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Ethereum receipt trie key has an invalid nibble",
                ));
            }
            groups[nibble].push(*entry);
        }
    }
    let mut children: [Option<Box<TrieNode>>; 16] = array::from_fn(|_| None);
    for (index, group) in groups.iter().enumerate() {
        if !group.is_empty() {
            children[index] = Some(Box::new(build_trie(group, depth + 1)?));
        }
    }
    let mut items = children
        .iter()
        .map(|child| child.as_deref().map_or_else(rlp_empty, node_reference))
        .collect::<Vec<_>>();
    items.push(
        value
            .as_ref()
            .map_or_else(rlp_empty, |value| rlp_bytes(value)),
    );
    let encoded = rlp_list(&items);
    Ok(TrieNode {
        encoded,
        kind: TrieNodeKind::Branch { children, value },
    })
}

fn collect_proof(
    node: &TrieNode,
    key: &[u8],
    depth: usize,
    proof: &mut Vec<Vec<u8>>,
) -> io::Result<()> {
    proof.push(node.encoded.clone());
    match &node.kind {
        TrieNodeKind::Leaf { path, value } => {
            if key.get(depth..) != Some(path.as_slice()) || value.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "target Ethereum receipt is absent from the reconstructed trie",
                ));
            }
            Ok(())
        }
        TrieNodeKind::Extension { path, child } => {
            if !key
                .get(depth..)
                .is_some_and(|remaining| remaining.starts_with(path))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "target Ethereum receipt diverges at a trie extension",
                ));
            }
            collect_proof(child, key, depth + path.len(), proof)
        }
        TrieNodeKind::Branch { children, value } => {
            if depth == key.len() {
                if value.is_none() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "target Ethereum receipt is absent at a trie branch",
                    ));
                }
                return Ok(());
            }
            let nibble = usize::from(key[depth]);
            let child = children
                .get(nibble)
                .and_then(|child| child.as_deref())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "target Ethereum receipt branch is absent",
                    )
                })?;
            collect_proof(child, key, depth + 1, proof)
        }
    }
}

fn common_prefix_len(entries: &[&TrieEntry], depth: usize) -> io::Result<usize> {
    let first = entries[0].key.get(depth..).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum receipt trie depth exceeds a key",
        )
    })?;
    let mut common = first.len();
    for entry in &entries[1..] {
        let key = entry.key.get(depth..).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum receipt trie depth exceeds a key",
            )
        })?;
        common = common.min(key.len()).min(
            first
                .iter()
                .zip(key)
                .take_while(|(left, right)| left == right)
                .count(),
        );
    }
    Ok(common)
}

fn node_reference(node: &TrieNode) -> Vec<u8> {
    if node.encoded.len() < 32 {
        node.encoded.clone()
    } else {
        rlp_bytes(&postfiat_bridge::ethereum_keccak256(&node.encoded))
    }
}

fn compact_path(nibbles: &[u8], leaf: bool) -> io::Result<Vec<u8>> {
    if nibbles.iter().any(|nibble| *nibble > 0x0f) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum trie path contains a non-nibble",
        ));
    }
    let odd = nibbles.len() % 2 == 1;
    let mut encoded = Vec::with_capacity((nibbles.len() + 2) / 2);
    let flag = if leaf { 2_u8 } else { 0_u8 };
    let mut index = 0;
    if odd {
        encoded.push(((flag + 1) << 4) | nibbles[0]);
        index = 1;
    } else {
        encoded.push(flag << 4);
    }
    while index < nibbles.len() {
        encoded.push((nibbles[index] << 4) | nibbles[index + 1]);
        index += 2;
    }
    Ok(encoded)
}

fn encode_receipt(receipt: &Value) -> io::Result<Vec<u8>> {
    let outcome = if let Some(status) = receipt.get("status") {
        quantity_bytes(status.as_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "receipt status is not a quantity",
            )
        })?)?
    } else {
        exact_hex::<32>(
            "pre-Byzantium receipt root",
            required_string(receipt, "root")?,
        )?
        .to_vec()
    };
    let cumulative_gas = quantity_bytes(required_string(receipt, "cumulativeGasUsed")?)?;
    let bloom = exact_hex::<256>("receipt logs bloom", required_string(receipt, "logsBloom")?)?;
    let logs = receipt
        .get("logs")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "receipt has no log array"))?;
    if logs.len() > MAX_LOGS_PER_RECEIPT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum receipt log count exceeds the bounded builder limit",
        ));
    }
    let logs = logs
        .iter()
        .map(|log| {
            let address = exact_hex::<20>("log address", required_string(log, "address")?)?;
            let topics = log.get("topics").and_then(Value::as_array).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "log has no topic array")
            })?;
            if topics.len() > 4 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Ethereum log has more than four topics",
                ));
            }
            let topics = topics
                .iter()
                .map(|topic| {
                    let topic = topic.as_str().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "log topic is not hexadecimal")
                    })?;
                    Ok(rlp_bytes(&exact_hex::<32>("log topic", topic)?))
                })
                .collect::<io::Result<Vec<_>>>()?;
            let data = hex_bytes("log data", required_string(log, "data")?)?;
            Ok(rlp_list(&[
                rlp_bytes(&address),
                rlp_list(&topics),
                rlp_bytes(&data),
            ]))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let payload = rlp_list(&[
        rlp_bytes(&outcome),
        rlp_bytes(&cumulative_gas),
        rlp_bytes(&bloom),
        rlp_list(&logs),
    ]);
    let receipt_type = receipt
        .get("type")
        .and_then(Value::as_str)
        .map(parse_quantity)
        .transpose()?
        .unwrap_or(0);
    let encoded = if receipt_type == 0 {
        payload
    } else {
        let receipt_type = u8::try_from(receipt_type).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum receipt type exceeds one byte",
            )
        })?;
        if receipt_type > 0x7f {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum receipt type is outside the EIP-2718 range",
            ));
        }
        let mut typed = Vec::with_capacity(payload.len() + 1);
        typed.push(receipt_type);
        typed.extend_from_slice(&payload);
        typed
    };
    if encoded.len() > ETHEREUM_RECEIPT_PROOF_MAX_RECEIPT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum receipt exceeds the protocol proof bound",
        ));
    }
    Ok(encoded)
}

fn rlp_u64(value: u64) -> Vec<u8> {
    rlp_bytes(&minimal_u64_bytes(value))
}

fn rlp_empty() -> Vec<u8> {
    vec![0x80]
}

fn rlp_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() == 1 && bytes[0] <= 0x7f {
        return bytes.to_vec();
    }
    if bytes.len() < 56 {
        let mut encoded = vec![0x80 + bytes.len() as u8];
        encoded.extend_from_slice(bytes);
        return encoded;
    }
    let length = minimal_usize_bytes(bytes.len());
    let mut encoded = vec![0xb7 + length.len() as u8];
    encoded.extend_from_slice(&length);
    encoded.extend_from_slice(bytes);
    encoded
}

fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload_len = items.iter().map(Vec::len).sum::<usize>();
    let mut encoded = if payload_len < 56 {
        vec![0xc0 + payload_len as u8]
    } else {
        let length = minimal_usize_bytes(payload_len);
        let mut prefix = vec![0xf7 + length.len() as u8];
        prefix.extend_from_slice(&length);
        prefix
    };
    for item in items {
        encoded.extend_from_slice(item);
    }
    encoded
}

fn bytes_to_nibbles(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .collect()
}

fn minimal_u64_bytes(value: u64) -> Vec<u8> {
    if value == 0 {
        return Vec::new();
    }
    let bytes = value.to_be_bytes();
    bytes[bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len())..]
        .to_vec()
}

fn minimal_usize_bytes(value: usize) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    bytes[bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len())..]
        .to_vec()
}

fn rpc_quantity(endpoint: &EthereumRpcEndpoint, method: &str, params: Value) -> io::Result<u64> {
    let value = ethereum_rpc_call(endpoint, method, params)?;
    parse_quantity(value.as_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum RPC quantity is not a string",
        )
    })?)
}

fn quantity_field(value: &Value, field: &'static str) -> io::Result<u64> {
    parse_quantity(required_string(value, field)?)
}

fn parse_quantity(value: &str) -> io::Result<u64> {
    let bytes = quantity_bytes(value)?;
    if bytes.len() > 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum quantity exceeds u64",
        ));
    }
    Ok(bytes
        .into_iter()
        .fold(0_u64, |result, byte| (result << 8) | u64::from(byte)))
}

fn quantity_bytes(value: &str) -> io::Result<Vec<u8>> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum quantity is missing 0x",
        )
    })?;
    if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum quantity is not minimally encoded",
        ));
    }
    if digits == "0" {
        return Ok(Vec::new());
    }
    let padded;
    let even = if digits.len() % 2 == 1 {
        padded = format!("0{digits}");
        padded.as_str()
    } else {
        digits
    };
    hex_to_bytes(even).map_err(invalid_data)
}

fn required_string<'a>(value: &'a Value, field: &'static str) -> io::Result<&'a str> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Ethereum object has no `{field}` string field"),
        )
    })
}

fn canonical_hash(field: &'static str, value: &str) -> io::Result<String> {
    Ok(format!(
        "0x{}",
        bytes_to_hex(&exact_hex::<32>(field, value)?)
    ))
}

fn hex_bytes(field: &'static str, value: &str) -> io::Result<Vec<u8>> {
    hex_to_bytes(value.strip_prefix("0x").ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, format!("{field} is missing 0x"))
    })?)
    .map_err(invalid_data)
}

fn exact_hex<const N: usize>(field: &'static str, value: &str) -> io::Result<[u8; N]> {
    hex_bytes(field, value)?
        .try_into()
        .map_err(|bytes: Vec<u8>| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{field} has {} bytes; expected {N}", bytes.len()),
            )
        })
}

fn write_json(path: &std::path::Path, value: &impl Serialize) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn invalid_data(error: impl std::fmt::Debug) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    use postfiat_types::{
        EthereumRouteVerificationPolicyV1, FastSwapCommitteeRootV1, LedgerState,
        PftlUniswapConsensusRouteState, PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT,
    };

    use super::*;

    #[test]
    fn reconstructs_multi_receipt_trie_and_proofs_every_index() {
        let receipts = (0_u64..260)
            .map(|index| TrieEntry {
                key: bytes_to_nibbles(&rlp_u64(index)),
                value: rlp_list(&[
                    rlp_bytes(&[1]),
                    rlp_bytes(&minimal_u64_bytes(index + 1)),
                    rlp_bytes(&[0; 256]),
                    rlp_list(&[]),
                ]),
            })
            .collect::<Vec<_>>();
        let refs = receipts.iter().collect::<Vec<_>>();
        let trie = build_trie(&refs, 0).expect("build receipt trie");
        let root = postfiat_bridge::ethereum_keccak256(&trie.encoded);

        for index in [0_u64, 1, 15, 127, 128, 255, 259] {
            let mut proof_nodes_rlp = Vec::new();
            let key = bytes_to_nibbles(&rlp_u64(index));
            collect_proof(&trie, &key, 0, &mut proof_nodes_rlp).expect("collect receipt proof");
            let proof = EthereumReceiptProofV1 {
                transaction_index: index,
                receipt_rlp: receipts[index as usize].value.clone(),
                proof_nodes_rlp,
            };
            let rejection = postfiat_bridge::verify_ethereum_receipt_log(root, &proof, 0)
                .expect_err("empty-log receipt has valid inclusion but no requested log");
            assert_eq!(rejection.code(), "ethereum_receipt_log_index_invalid");
        }

        let mut tampered = receipts[128].value.clone();
        *tampered.last_mut().expect("receipt byte") ^= 1;
        let mut proof_nodes_rlp = Vec::new();
        collect_proof(
            &trie,
            &bytes_to_nibbles(&rlp_u64(128)),
            0,
            &mut proof_nodes_rlp,
        )
        .expect("collect tampered proof path");
        let proof = EthereumReceiptProofV1 {
            transaction_index: 128,
            receipt_rlp: tampered,
            proof_nodes_rlp,
        };
        assert_eq!(
            postfiat_bridge::verify_ethereum_receipt_log(root, &proof, 0)
                .expect_err("tampered receipt must not verify")
                .code(),
            "ethereum_receipt_value_mismatch"
        );
    }

    #[test]
    fn matches_anvil_1_7_1_three_eip1559_receipt_root() {
        let zero_bloom = format!("0x{}", "00".repeat(256));
        let receipts = ["0x5208", "0xa410", "0xf618"]
            .into_iter()
            .map(|cumulative_gas| {
                serde_json::json!({
                    "type": "0x2",
                    "status": "0x1",
                    "cumulativeGasUsed": cumulative_gas,
                    "logs": [],
                    "logsBloom": zero_bloom,
                })
            })
            .enumerate()
            .map(|(index, receipt)| {
                Ok(TrieEntry {
                    key: bytes_to_nibbles(&rlp_u64(u64::try_from(index).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "test index exceeds u64")
                    })?)),
                    value: encode_receipt(&receipt)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()
            .expect("encode captured Anvil receipts");
        let refs = receipts.iter().collect::<Vec<_>>();
        let trie = build_trie(&refs, 0).expect("build captured Anvil receipt trie");
        assert_eq!(
            postfiat_bridge::ethereum_keccak256(&trie.encoded),
            exact_hex::<32>(
                "captured Anvil receipts root",
                "0x25e6b7af647c519a27cc13276a1e6abc46154b51414d174b072698df1f6c19df"
            )
            .expect("captured Anvil root")
        );
    }

    #[test]
    fn production_builder_binds_governed_route_and_captured_anvil_block() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-ethereum-receipt-builder-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create receipt builder test dir");
        let route_id = "anvil-receipt-proof";
        let mut ledger = LedgerState::empty();
        ledger
            .pftl_uniswap_routes
            .push(PftlUniswapConsensusRouteState {
                route_id: route_id.to_string(),
                route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
                route_config_digest: "11".repeat(48),
                route_trust_class: postfiat_bridge::ROUTE_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
                native_nav_asset_id: "22".repeat(48),
                settlement_asset_id: "23".repeat(48),
                handoff_controller: format!("0x{}", "33".repeat(20)),
                settlement_adapter: format!("0x{}", "34".repeat(20)),
                wrapped_navcoin_token: format!("0x{}", "35".repeat(20)),
                ethereum_chain_id: 31_337,
                route_supply_cap_atoms: 1_000,
                packet_notional_cap_atoms: 100,
                latest_finalized_nav_epoch: 1,
                return_finality_blocks: 12,
                ethereum_verification_policy: Some(EthereumRouteVerificationPolicyV1 {
                    authority_epoch: 1,
                    committee_root: FastSwapCommitteeRootV1([0x41; 48]),
                    minimum_confirmations: 12,
                    handoff_controller_code_hash: [0x42; 32],
                    wrapped_navcoin_code_hash: [0x43; 32],
                }),
                authorized_valid_supply_atoms: 0,
                pftl_spendable_supply_atoms: 0,
                native_spendable_balances_atoms: BTreeMap::new(),
                ethereum_spendable_supply_atoms: 0,
                other_registered_venue_supply_atoms: 0,
                outstanding_bridge_claims_atoms: 0,
                pending_return_import_claims_atoms: 0,
                settlement_reserve_atoms: 0,
                primary_subscription_nonces: BTreeMap::new(),
                export_packets: BTreeMap::new(),
                export_nonces: BTreeMap::new(),
                return_imports: BTreeMap::new(),
                paused: false,
            });
        NodeStore::new(&root)
            .write_ledger(&ledger)
            .expect("write governed receipt route");

        let (rpc, rpc_thread) = captured_anvil_rpc();
        let proof_file = root.join("receipt-proof.json");
        let artifact = build_ethereum_receipt_proof(EthereumReceiptProofBuildOptions {
            data_dir: root.clone(),
            route_id: route_id.to_string(),
            ethereum_rpc: rpc,
            transaction_hash: "0x42729941c7c3408e245891806dba06c1a189c611d5e3a4b380da1981f27f2470"
                .to_string(),
            proof_file: proof_file.clone(),
        })
        .expect("build proof from captured Anvil block");
        rpc_thread.join().expect("captured Anvil RPC server");
        assert!(proof_file.is_file());
        assert_eq!(artifact.proof.transaction_index, 1);
        let receipts_root = exact_hex::<32>("artifact receipts root", &artifact.receipts_root)
            .expect("artifact receipts root");
        assert_eq!(
            postfiat_bridge::verify_ethereum_receipt_log(receipts_root, &artifact.proof, 0)
                .expect_err("captured transfer receipt has no log")
                .code(),
            "ethereum_receipt_log_index_invalid"
        );
        std::fs::remove_dir_all(root).expect("remove receipt builder test dir");
    }

    fn captured_anvil_rpc() -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind captured Anvil RPC");
        let address = listener.local_addr().expect("captured Anvil RPC address");
        let block_hash = "0xe9a0232d4fd6d17f470d797d7c7c50cf606a4f96a542272347811f52ce8a5361";
        let receipts_root = "0x25e6b7af647c519a27cc13276a1e6abc46154b51414d174b072698df1f6c19df";
        let transaction_hashes = [
            "0x37eac23662b2822928a6dc8fc09af37777c9aac186a0e0f94fa1352b368b1bba",
            "0x42729941c7c3408e245891806dba06c1a189c611d5e3a4b380da1981f27f2470",
            "0xcc3dab9b8206d3e15e0f91745cbb76619ba782eb9277aa91003ffcaaf79a13dd",
        ];
        let receipts = transaction_hashes
            .iter()
            .zip(["0x5208", "0xa410", "0xf618"])
            .enumerate()
            .map(|(index, (transaction_hash, cumulative_gas))| {
                captured_anvil_receipt(
                    u64::try_from(index).expect("captured receipt index"),
                    transaction_hash,
                    cumulative_gas,
                    block_hash,
                )
            })
            .collect::<Vec<_>>();
        let handle = std::thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().expect("accept captured Anvil request");
                let request = read_http_json_request(&mut stream);
                let method = request
                    .get("method")
                    .and_then(Value::as_str)
                    .expect("captured Anvil method");
                let result = match method {
                    "eth_chainId" => Value::String("0x7a69".to_string()),
                    "eth_getTransactionReceipt" => receipts[1].clone(),
                    "eth_getBlockByHash" => serde_json::json!({
                        "hash": block_hash,
                        "number": "0x1",
                        "receiptsRoot": receipts_root,
                        "transactions": transaction_hashes,
                    }),
                    "eth_getBlockReceipts" => Value::Array(receipts.clone()),
                    other => panic!("unexpected captured Anvil method {other}"),
                };
                let body = serde_json::to_vec(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": result,
                }))
                .expect("serialize captured Anvil response");
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("write captured Anvil response header");
                stream
                    .write_all(&body)
                    .expect("write captured Anvil response body");
            }
        });
        (format!("http://{address}"), handle)
    }

    fn captured_anvil_receipt(
        index: u64,
        transaction_hash: &str,
        cumulative_gas: &str,
        block_hash: &str,
    ) -> Value {
        serde_json::json!({
            "type": "0x2",
            "status": "0x1",
            "cumulativeGasUsed": cumulative_gas,
            "logs": [],
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "transactionHash": transaction_hash,
            "transactionIndex": format!("0x{index:x}"),
            "blockHash": block_hash,
            "blockNumber": "0x1",
        })
    }

    fn read_http_json_request(stream: &mut TcpStream) -> Value {
        let mut request = Vec::new();
        let mut chunk = [0_u8; 2048];
        loop {
            let count = stream
                .read(&mut chunk)
                .expect("read captured Anvil request");
            assert!(count > 0, "captured Anvil request closed before body");
            request.extend_from_slice(&chunk[..count]);
            let Some(header_index) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                continue;
            };
            let body_start = header_index + 4;
            let headers = std::str::from_utf8(&request[..body_start]).expect("request headers");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .expect("request Content-Length");
            if request.len() >= body_start + content_length {
                return serde_json::from_slice(&request[body_start..body_start + content_length])
                    .expect("parse captured Anvil request");
            }
        }
    }
}
