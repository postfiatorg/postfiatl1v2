use postfiat_types::EthereumReceiptProofV1;
use sha3::{Digest, Keccak256};

const MAX_PROOF_NODES: usize = 64;
const MAX_NODE_BYTES: usize = 64 * 1024;
const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
const MAX_RLP_DEPTH: usize = 8;
const MAX_RLP_ITEMS: usize = 4_096;
const ETHEREUM_LOG_BLOOM_BYTES: usize = 256;
const ETHEREUM_ADDRESS_BYTES: usize = 20;
const ETHEREUM_TOPIC_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumLogV1 {
    pub emitter: [u8; ETHEREUM_ADDRESS_BYTES],
    pub topics: Vec<[u8; ETHEREUM_TOPIC_BYTES]>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketConsumedEventV1 {
    pub controller: [u8; 20],
    pub packet_digest: [u8; 32],
    pub source_packet_commitment: [u8; 32],
    pub recipient: [u8; 20],
    pub route_config_commitment: [u8; 32],
    pub source_receipt_commitment: [u8; 32],
    pub route_trust_class: [u8; 32],
    pub mint_amount_atoms: u64,
    pub settlement_amount_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketCancelledEventV1 {
    pub controller: [u8; 20],
    pub packet_digest: [u8; 32],
    pub source_packet_commitment: [u8; 32],
    pub source_receipt_commitment: [u8; 32],
    pub deadline: u64,
    pub cancelled_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnBurnedEventV1 {
    pub controller: [u8; 20],
    pub return_burn_id: [u8; 32],
    pub ethereum_sender: [u8; 20],
    pub return_nonce: [u8; 32],
    pub pftl_recipient: String,
    pub native_nav_asset_id: Vec<u8>,
    pub amount_atoms: u64,
    pub ethereum_chain_id: u64,
    pub bridge_controller: [u8; 20],
    pub wrapped_navcoin: [u8; 20],
    pub burn_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumProofError {
    code: &'static str,
    message: String,
}

impl EthereumProofError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for EthereumProofError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EthereumProofError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Rlp<'a> {
    Bytes(&'a [u8]),
    List(Vec<Rlp<'a>>),
}

impl<'a> Rlp<'a> {
    fn bytes(&self, field: &'static str) -> Result<&'a [u8], EthereumProofError> {
        match self {
            Self::Bytes(bytes) => Ok(bytes),
            Self::List(_) => Err(EthereumProofError::new(
                "ethereum_rlp_type_mismatch",
                format!("{field} must be RLP bytes"),
            )),
        }
    }

    fn list(&self, field: &'static str) -> Result<&[Rlp<'a>], EthereumProofError> {
        match self {
            Self::List(items) => Ok(items),
            Self::Bytes(_) => Err(EthereumProofError::new(
                "ethereum_rlp_type_mismatch",
                format!("{field} must be an RLP list"),
            )),
        }
    }
}

pub fn verify_ethereum_receipt_log(
    receipts_root: [u8; 32],
    proof: &EthereumReceiptProofV1,
    log_index: usize,
) -> Result<EthereumLogV1, EthereumProofError> {
    validate_proof_bounds(proof)?;
    let key = bytes_to_nibbles(&rlp_encode_u64(proof.transaction_index));
    verify_merkle_patricia_value(
        receipts_root,
        &key,
        &proof.receipt_rlp,
        &proof.proof_nodes_rlp,
    )?;
    decode_successful_receipt_log(&proof.receipt_rlp, log_index)
}

pub fn verify_packet_consumed_event(
    log: &EthereumLogV1,
    expected: &PacketConsumedEventV1,
) -> Result<(), EthereumProofError> {
    let actual = decode_packet_consumed_event(log, expected.controller)?;
    require_exact_event("PacketConsumed", &actual, expected)
}

pub fn decode_packet_consumed_event(
    log: &EthereumLogV1,
    expected_controller: [u8; 20],
) -> Result<PacketConsumedEventV1, EthereumProofError> {
    require_event_header(
        log,
        expected_controller,
        "PacketConsumed(bytes32,bytes32,address,bytes32,bytes32,bytes32,uint256,uint256)",
        4,
    )?;
    Ok(PacketConsumedEventV1 {
        controller: log.emitter,
        packet_digest: log.topics[1],
        source_packet_commitment: log.topics[2],
        recipient: topic_address(&log.topics[3], "PacketConsumed.recipient")?,
        route_config_commitment: data_word(&log.data, 0, 5, "PacketConsumed.route_config")?,
        source_receipt_commitment: data_word(&log.data, 1, 5, "PacketConsumed.source_receipt")?,
        route_trust_class: data_word(&log.data, 2, 5, "PacketConsumed.trust_class")?,
        mint_amount_atoms: data_u64(&log.data, 3, 5, "PacketConsumed.mint_amount")?,
        settlement_amount_atoms: data_u64(&log.data, 4, 5, "PacketConsumed.settlement_amount")?,
    })
}

pub fn verify_packet_cancelled_event(
    log: &EthereumLogV1,
    expected: &PacketCancelledEventV1,
) -> Result<(), EthereumProofError> {
    let actual = decode_packet_cancelled_event(log, expected.controller)?;
    require_exact_event("PacketCancelled", &actual, expected)
}

pub fn decode_packet_cancelled_event(
    log: &EthereumLogV1,
    expected_controller: [u8; 20],
) -> Result<PacketCancelledEventV1, EthereumProofError> {
    require_event_header(
        log,
        expected_controller,
        "PacketCancelled(bytes32,bytes32,bytes32,uint64,uint64)",
        4,
    )?;
    Ok(PacketCancelledEventV1 {
        controller: log.emitter,
        packet_digest: log.topics[1],
        source_packet_commitment: log.topics[2],
        source_receipt_commitment: log.topics[3],
        deadline: data_u64(&log.data, 0, 2, "PacketCancelled.deadline")?,
        cancelled_at: data_u64(&log.data, 1, 2, "PacketCancelled.cancelled_at")?,
    })
}

pub fn verify_return_burned_event(
    log: &EthereumLogV1,
    expected: &ReturnBurnedEventV1,
) -> Result<(), EthereumProofError> {
    let actual = decode_return_burned_event(log, expected.controller)?;
    require_exact_event("ReturnBurned", &actual, expected)
}

pub fn decode_return_burned_event(
    log: &EthereumLogV1,
    expected_controller: [u8; 20],
) -> Result<ReturnBurnedEventV1, EthereumProofError> {
    require_event_header(
        log,
        expected_controller,
        "ReturnBurned(bytes32,address,bytes32,string,bytes,uint256,uint256,address,address,uint256)",
        4,
    )?;
    const HEAD_WORDS: usize = 7;
    require_abi_data_minimum(&log.data, HEAD_WORDS, "ReturnBurned")?;
    let (recipient, recipient_end) = dynamic_bytes(
        &log.data,
        HEAD_WORDS,
        0,
        HEAD_WORDS * 32,
        "ReturnBurned.pftl_recipient",
    )?;
    let (native_asset, native_asset_end) = dynamic_bytes(
        &log.data,
        HEAD_WORDS,
        1,
        recipient_end,
        "ReturnBurned.native_nav_asset_id",
    )?;
    if native_asset_end != log.data.len() {
        return Err(EthereumProofError::new(
            "ethereum_event_abi_noncanonical",
            "ReturnBurned has trailing ABI bytes",
        ));
    }
    let pftl_recipient = std::str::from_utf8(recipient).map_err(|_| {
        EthereumProofError::new(
            "ethereum_event_text_invalid",
            "ReturnBurned PFTL recipient is not UTF-8",
        )
    })?;
    Ok(ReturnBurnedEventV1 {
        controller: log.emitter,
        return_burn_id: log.topics[1],
        ethereum_sender: topic_address(&log.topics[2], "ReturnBurned.ethereum_sender")?,
        return_nonce: log.topics[3],
        pftl_recipient: pftl_recipient.to_string(),
        native_nav_asset_id: native_asset.to_vec(),
        amount_atoms: data_head_u64(&log.data, 2, HEAD_WORDS, "ReturnBurned.amount_atoms")?,
        ethereum_chain_id: data_head_u64(
            &log.data,
            3,
            HEAD_WORDS,
            "ReturnBurned.ethereum_chain_id",
        )?,
        bridge_controller: data_address(
            &log.data,
            4,
            HEAD_WORDS,
            "ReturnBurned.bridge_controller",
        )?,
        wrapped_navcoin: data_address(&log.data, 5, HEAD_WORDS, "ReturnBurned.wrapped_navcoin")?,
        burn_height: data_head_u64(&log.data, 6, HEAD_WORDS, "ReturnBurned.burn_height")?,
    })
}

pub fn ethereum_keccak256(bytes: &[u8]) -> [u8; 32] {
    Keccak256::digest(bytes).into()
}

fn require_exact_event<T: PartialEq>(
    name: &'static str,
    actual: &T,
    expected: &T,
) -> Result<(), EthereumProofError> {
    if actual == expected {
        Ok(())
    } else {
        Err(EthereumProofError::new(
            "ethereum_event_binding_mismatch",
            format!("{name} does not match the route-bound expected fields"),
        ))
    }
}

fn require_event_header(
    log: &EthereumLogV1,
    expected_emitter: [u8; 20],
    signature: &'static str,
    topic_count: usize,
) -> Result<(), EthereumProofError> {
    if log.emitter != expected_emitter {
        return Err(EthereumProofError::new(
            "ethereum_event_emitter_mismatch",
            "Ethereum log emitter does not match the governed bridge controller",
        ));
    }
    if log.topics.len() != topic_count {
        return Err(EthereumProofError::new(
            "ethereum_event_topic_count_invalid",
            format!(
                "Ethereum event has {} topics; expected {topic_count}",
                log.topics.len()
            ),
        ));
    }
    let expected_topic: [u8; 32] = Keccak256::digest(signature.as_bytes()).into();
    if log.topics[0] != expected_topic {
        return Err(EthereumProofError::new(
            "ethereum_event_topic_mismatch",
            "Ethereum event signature topic does not match the required transition",
        ));
    }
    Ok(())
}

fn require_abi_data_minimum(
    data: &[u8],
    head_words: usize,
    event: &'static str,
) -> Result<(), EthereumProofError> {
    let minimum = head_words.checked_mul(32).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{event} ABI head length overflows"),
        )
    })?;
    if data.len() < minimum {
        return Err(EthereumProofError::new(
            "ethereum_event_abi_truncated",
            format!(
                "{event} ABI data has {} bytes; expected at least {minimum}",
                data.len()
            ),
        ));
    }
    Ok(())
}

fn data_word(
    data: &[u8],
    index: usize,
    exact_words: usize,
    field: &'static str,
) -> Result<[u8; 32], EthereumProofError> {
    let expected_length = exact_words.checked_mul(32).ok_or_else(|| {
        EthereumProofError::new("ethereum_event_abi_overflow", "ABI word length overflows")
    })?;
    if data.len() != expected_length {
        return Err(EthereumProofError::new(
            "ethereum_event_abi_size_invalid",
            format!(
                "{field} event data has {} bytes; expected {expected_length}",
                data.len()
            ),
        ));
    }
    data_word_at(data, index, field)
}

fn data_word_at(
    data: &[u8],
    index: usize,
    field: &'static str,
) -> Result<[u8; 32], EthereumProofError> {
    let start = index.checked_mul(32).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} offset overflows"),
        )
    })?;
    let end = start.checked_add(32).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} end overflows"),
        )
    })?;
    data.get(start..end)
        .ok_or_else(|| {
            EthereumProofError::new(
                "ethereum_event_abi_truncated",
                format!("{field} word is outside event data"),
            )
        })?
        .try_into()
        .map_err(|_| {
            EthereumProofError::new(
                "ethereum_event_abi_truncated",
                format!("{field} word is not 32 bytes"),
            )
        })
}

fn data_u64(
    data: &[u8],
    index: usize,
    exact_words: usize,
    field: &'static str,
) -> Result<u64, EthereumProofError> {
    let word = if data.len() == exact_words.saturating_mul(32) {
        data_word_at(data, index, field)?
    } else {
        return Err(EthereumProofError::new(
            "ethereum_event_abi_size_invalid",
            format!(
                "{field} event data has {} bytes; expected {}",
                data.len(),
                exact_words.saturating_mul(32)
            ),
        ));
    };
    word_u64(&word, field)
}

fn data_head_u64(
    data: &[u8],
    index: usize,
    head_words: usize,
    field: &'static str,
) -> Result<u64, EthereumProofError> {
    require_abi_data_minimum(data, head_words, field)?;
    word_u64(&data_word_at(data, index, field)?, field)
}

fn word_u64(word: &[u8; 32], field: &'static str) -> Result<u64, EthereumProofError> {
    if word[..24].iter().any(|byte| *byte != 0) {
        return Err(EthereumProofError::new(
            "ethereum_event_integer_overflow",
            format!("{field} exceeds u64"),
        ));
    }
    let mut suffix = [0_u8; 8];
    suffix.copy_from_slice(&word[24..]);
    Ok(u64::from_be_bytes(suffix))
}

fn topic_address(topic: &[u8; 32], field: &'static str) -> Result<[u8; 20], EthereumProofError> {
    abi_word_address(topic, field)
}

fn data_address(
    data: &[u8],
    index: usize,
    head_words: usize,
    field: &'static str,
) -> Result<[u8; 20], EthereumProofError> {
    require_abi_data_minimum(data, head_words, field)?;
    abi_word_address(&data_word_at(data, index, field)?, field)
}

fn abi_word_address(word: &[u8; 32], field: &'static str) -> Result<[u8; 20], EthereumProofError> {
    if word[..12].iter().any(|byte| *byte != 0) {
        return Err(EthereumProofError::new(
            "ethereum_event_address_padding_invalid",
            format!("{field} has nonzero ABI address padding"),
        ));
    }
    word[12..].try_into().map_err(|_| {
        EthereumProofError::new(
            "ethereum_event_address_invalid",
            format!("{field} is not a 20-byte address"),
        )
    })
}

fn dynamic_bytes<'a>(
    data: &'a [u8],
    head_words: usize,
    offset_word: usize,
    expected_offset: usize,
    field: &'static str,
) -> Result<(&'a [u8], usize), EthereumProofError> {
    let offset_word = data_word_at(data, offset_word, field)?;
    let offset_u64 = word_u64(&offset_word, field)?;
    let offset = usize::try_from(offset_u64).map_err(|_| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} offset exceeds usize"),
        )
    })?;
    if offset != expected_offset || offset % 32 != 0 || offset < head_words.saturating_mul(32) {
        return Err(EthereumProofError::new(
            "ethereum_event_abi_noncanonical",
            format!("{field} has a noncanonical dynamic offset"),
        ));
    }
    let length_end = offset.checked_add(32).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} length end overflows"),
        )
    })?;
    let length_word: [u8; 32] = data
        .get(offset..length_end)
        .ok_or_else(|| {
            EthereumProofError::new(
                "ethereum_event_abi_truncated",
                format!("{field} dynamic length is outside event data"),
            )
        })?
        .try_into()
        .map_err(|_| {
            EthereumProofError::new(
                "ethereum_event_abi_truncated",
                format!("{field} dynamic length is not 32 bytes"),
            )
        })?;
    let length = usize::try_from(word_u64(&length_word, field)?).map_err(|_| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} length exceeds usize"),
        )
    })?;
    let start = offset.checked_add(32).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} start overflows"),
        )
    })?;
    let end = start.checked_add(length).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} end overflows"),
        )
    })?;
    let padded_length = length.div_ceil(32).checked_mul(32).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} padded length overflows"),
        )
    })?;
    let padded_end = start.checked_add(padded_length).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_overflow",
            format!("{field} padded end overflows"),
        )
    })?;
    let value = data.get(start..end).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_truncated",
            format!("{field} dynamic value is outside event data"),
        )
    })?;
    let padding = data.get(end..padded_end).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_event_abi_truncated",
            format!("{field} dynamic padding is outside event data"),
        )
    })?;
    if padding.iter().any(|byte| *byte != 0) {
        return Err(EthereumProofError::new(
            "ethereum_event_abi_noncanonical",
            format!("{field} has nonzero dynamic padding"),
        ));
    }
    Ok((value, padded_end))
}

fn validate_proof_bounds(proof: &EthereumReceiptProofV1) -> Result<(), EthereumProofError> {
    if proof.receipt_rlp.is_empty() || proof.receipt_rlp.len() > MAX_RECEIPT_BYTES {
        return Err(EthereumProofError::new(
            "ethereum_receipt_size_invalid",
            format!(
                "receipt size {} is outside 1..={MAX_RECEIPT_BYTES}",
                proof.receipt_rlp.len()
            ),
        ));
    }
    if proof.proof_nodes_rlp.is_empty() || proof.proof_nodes_rlp.len() > MAX_PROOF_NODES {
        return Err(EthereumProofError::new(
            "ethereum_receipt_proof_length_invalid",
            format!(
                "proof node count {} is outside 1..={MAX_PROOF_NODES}",
                proof.proof_nodes_rlp.len()
            ),
        ));
    }
    for node in &proof.proof_nodes_rlp {
        if node.is_empty() || node.len() > MAX_NODE_BYTES {
            return Err(EthereumProofError::new(
                "ethereum_receipt_proof_node_size_invalid",
                format!(
                    "proof node size {} is outside 1..={MAX_NODE_BYTES}",
                    node.len()
                ),
            ));
        }
    }
    Ok(())
}

fn verify_merkle_patricia_value(
    root: [u8; 32],
    key: &[u8],
    expected_value: &[u8],
    proof_nodes: &[Vec<u8>],
) -> Result<(), EthereumProofError> {
    let mut expected_reference = root.to_vec();
    let mut key_offset = 0_usize;

    for (proof_index, encoded_node) in proof_nodes.iter().enumerate() {
        verify_node_reference(&expected_reference, encoded_node, proof_index)?;
        let node = parse_rlp_exact(encoded_node)?;
        let fields = node.list("receipt trie node")?;
        match fields.len() {
            17 => {
                if key_offset == key.len() {
                    let value = fields[16].bytes("branch value")?;
                    if value != expected_value {
                        return Err(EthereumProofError::new(
                            "ethereum_receipt_value_mismatch",
                            "branch value does not match the supplied receipt",
                        ));
                    }
                    if proof_index + 1 != proof_nodes.len() {
                        return Err(EthereumProofError::new(
                            "ethereum_receipt_proof_trailing_nodes",
                            "proof contains nodes after its terminal branch",
                        ));
                    }
                    return Ok(());
                }
                let nibble = usize::from(key[key_offset]);
                let child = fields[nibble].bytes("branch child")?;
                if child.is_empty() {
                    return Err(EthereumProofError::new(
                        "ethereum_receipt_proof_missing_child",
                        "receipt trie branch has no child for the transaction key",
                    ));
                }
                expected_reference = child.to_vec();
                key_offset += 1;
            }
            2 => {
                let encoded_path = fields[0].bytes("extension or leaf path")?;
                let (is_leaf, path) = decode_hex_prefix_path(encoded_path)?;
                let remaining = key.get(key_offset..).ok_or_else(|| {
                    EthereumProofError::new(
                        "ethereum_receipt_proof_path_mismatch",
                        "receipt trie key offset exceeds key length",
                    )
                })?;
                if !remaining.starts_with(&path) {
                    return Err(EthereumProofError::new(
                        "ethereum_receipt_proof_path_mismatch",
                        "receipt trie compact path does not match the transaction key",
                    ));
                }
                key_offset += path.len();
                if is_leaf {
                    if key_offset != key.len() {
                        return Err(EthereumProofError::new(
                            "ethereum_receipt_proof_path_mismatch",
                            "receipt trie leaf does not consume the complete transaction key",
                        ));
                    }
                    let value = fields[1].bytes("leaf value")?;
                    if value != expected_value {
                        return Err(EthereumProofError::new(
                            "ethereum_receipt_value_mismatch",
                            "leaf value does not match the supplied receipt",
                        ));
                    }
                    if proof_index + 1 != proof_nodes.len() {
                        return Err(EthereumProofError::new(
                            "ethereum_receipt_proof_trailing_nodes",
                            "proof contains nodes after its terminal leaf",
                        ));
                    }
                    return Ok(());
                }
                let child = fields[1].bytes("extension child")?;
                if child.is_empty() {
                    return Err(EthereumProofError::new(
                        "ethereum_receipt_proof_missing_child",
                        "receipt trie extension has an empty child reference",
                    ));
                }
                expected_reference = child.to_vec();
            }
            count => {
                return Err(EthereumProofError::new(
                    "ethereum_receipt_proof_node_shape_invalid",
                    format!("receipt trie node contains {count} fields; expected 2 or 17"),
                ));
            }
        }
    }

    Err(EthereumProofError::new(
        "ethereum_receipt_proof_incomplete",
        "receipt trie proof ended before reaching a terminal value",
    ))
}

fn verify_node_reference(
    expected_reference: &[u8],
    encoded_node: &[u8],
    proof_index: usize,
) -> Result<(), EthereumProofError> {
    let matches = match expected_reference.len() {
        32 => Keccak256::digest(encoded_node).as_slice() == expected_reference,
        1..=31 => encoded_node == expected_reference,
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(EthereumProofError::new(
            "ethereum_receipt_proof_node_hash_mismatch",
            format!("proof node {proof_index} does not match its parent reference"),
        ))
    }
}

fn decode_hex_prefix_path(encoded: &[u8]) -> Result<(bool, Vec<u8>), EthereumProofError> {
    if encoded.is_empty() {
        return Err(EthereumProofError::new(
            "ethereum_receipt_compact_path_invalid",
            "receipt trie compact path is empty",
        ));
    }
    let nibbles = bytes_to_nibbles(encoded);
    let flag = nibbles[0];
    if flag > 3 {
        return Err(EthereumProofError::new(
            "ethereum_receipt_compact_path_invalid",
            "receipt trie compact path has an invalid flag nibble",
        ));
    }
    let odd = flag & 1 == 1;
    let is_leaf = flag & 2 == 2;
    let path_start = if odd {
        1
    } else {
        if nibbles.get(1) != Some(&0) {
            return Err(EthereumProofError::new(
                "ethereum_receipt_compact_path_invalid",
                "even receipt trie compact path has a nonzero padding nibble",
            ));
        }
        2
    };
    Ok((is_leaf, nibbles[path_start..].to_vec()))
}

fn decode_successful_receipt_log(
    encoded_receipt: &[u8],
    log_index: usize,
) -> Result<EthereumLogV1, EthereumProofError> {
    let payload = if encoded_receipt.first().is_some_and(|byte| *byte <= 0x7f) {
        if encoded_receipt[0] == 0 {
            return Err(EthereumProofError::new(
                "ethereum_receipt_type_invalid",
                "typed Ethereum receipt has a zero transaction type",
            ));
        }
        &encoded_receipt[1..]
    } else {
        encoded_receipt
    };
    let receipt = parse_rlp_exact(payload)?;
    let fields = receipt.list("Ethereum receipt")?;
    if fields.len() != 4 {
        return Err(EthereumProofError::new(
            "ethereum_receipt_shape_invalid",
            format!(
                "Ethereum receipt contains {} fields; expected 4",
                fields.len()
            ),
        ));
    }
    let status = fields[0].bytes("receipt status")?;
    if status != [1] {
        return Err(EthereumProofError::new(
            "ethereum_receipt_not_successful",
            "receipt status is not successful or uses an unsupported pre-Byzantium state root",
        ));
    }
    let _cumulative_gas = fields[1].bytes("receipt cumulative gas")?;
    let bloom = fields[2].bytes("receipt logs bloom")?;
    if bloom.len() != ETHEREUM_LOG_BLOOM_BYTES {
        return Err(EthereumProofError::new(
            "ethereum_receipt_bloom_size_invalid",
            format!(
                "receipt logs bloom has {} bytes; expected {ETHEREUM_LOG_BLOOM_BYTES}",
                bloom.len()
            ),
        ));
    }
    let logs = fields[3].list("receipt logs")?;
    let log = logs.get(log_index).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_receipt_log_index_invalid",
            format!(
                "receipt has {} logs but index {log_index} was requested",
                logs.len()
            ),
        )
    })?;
    let log_fields = log.list("Ethereum log")?;
    if log_fields.len() != 3 {
        return Err(EthereumProofError::new(
            "ethereum_receipt_log_shape_invalid",
            format!(
                "Ethereum log contains {} fields; expected 3",
                log_fields.len()
            ),
        ));
    }
    let emitter_bytes = log_fields[0].bytes("log emitter")?;
    let emitter = emitter_bytes.try_into().map_err(|_| {
        EthereumProofError::new(
            "ethereum_receipt_log_emitter_invalid",
            format!(
                "log emitter has {} bytes; expected {ETHEREUM_ADDRESS_BYTES}",
                emitter_bytes.len()
            ),
        )
    })?;
    let topic_items = log_fields[1].list("log topics")?;
    let mut topics = Vec::with_capacity(topic_items.len());
    for topic in topic_items {
        let bytes = topic.bytes("log topic")?;
        let topic = bytes.try_into().map_err(|_| {
            EthereumProofError::new(
                "ethereum_receipt_log_topic_invalid",
                format!(
                    "log topic has {} bytes; expected {ETHEREUM_TOPIC_BYTES}",
                    bytes.len()
                ),
            )
        })?;
        topics.push(topic);
    }
    Ok(EthereumLogV1 {
        emitter,
        topics,
        data: log_fields[2].bytes("log data")?.to_vec(),
    })
}

fn parse_rlp_exact(input: &[u8]) -> Result<Rlp<'_>, EthereumProofError> {
    let mut item_count = 0_usize;
    let (item, consumed) = parse_rlp(input, 0, &mut item_count)?;
    if consumed != input.len() {
        return Err(EthereumProofError::new(
            "ethereum_rlp_trailing_bytes",
            "RLP value contains trailing bytes",
        ));
    }
    Ok(item)
}

fn parse_rlp<'a>(
    input: &'a [u8],
    depth: usize,
    item_count: &mut usize,
) -> Result<(Rlp<'a>, usize), EthereumProofError> {
    if depth > MAX_RLP_DEPTH {
        return Err(EthereumProofError::new(
            "ethereum_rlp_depth_exceeded",
            format!("RLP nesting exceeds {MAX_RLP_DEPTH}"),
        ));
    }
    *item_count = item_count.checked_add(1).ok_or_else(|| {
        EthereumProofError::new(
            "ethereum_rlp_item_limit_exceeded",
            "RLP item count overflow",
        )
    })?;
    if *item_count > MAX_RLP_ITEMS {
        return Err(EthereumProofError::new(
            "ethereum_rlp_item_limit_exceeded",
            format!("RLP item count exceeds {MAX_RLP_ITEMS}"),
        ));
    }
    let prefix = *input
        .first()
        .ok_or_else(|| EthereumProofError::new("ethereum_rlp_truncated", "RLP value is empty"))?;
    match prefix {
        0x00..=0x7f => Ok((Rlp::Bytes(&input[..1]), 1)),
        0x80..=0xb7 => {
            let length = usize::from(prefix - 0x80);
            let end = 1_usize
                .checked_add(length)
                .ok_or_else(rlp_length_overflow)?;
            let bytes = input.get(1..end).ok_or_else(rlp_truncated)?;
            if length == 1 && bytes[0] <= 0x7f {
                return Err(EthereumProofError::new(
                    "ethereum_rlp_noncanonical",
                    "single byte below 0x80 must use its byte encoding",
                ));
            }
            Ok((Rlp::Bytes(bytes), end))
        }
        0xb8..=0xbf => {
            let (length, payload_start) = decode_long_length(input, prefix - 0xb7)?;
            if length < 56 {
                return Err(EthereumProofError::new(
                    "ethereum_rlp_noncanonical",
                    "long-form RLP bytes encode a payload shorter than 56 bytes",
                ));
            }
            let end = payload_start
                .checked_add(length)
                .ok_or_else(rlp_length_overflow)?;
            Ok((
                Rlp::Bytes(input.get(payload_start..end).ok_or_else(rlp_truncated)?),
                end,
            ))
        }
        0xc0..=0xf7 => {
            let length = usize::from(prefix - 0xc0);
            parse_rlp_list(input, 1, length, depth, item_count)
        }
        0xf8..=0xff => {
            let (length, payload_start) = decode_long_length(input, prefix - 0xf7)?;
            if length < 56 {
                return Err(EthereumProofError::new(
                    "ethereum_rlp_noncanonical",
                    "long-form RLP list encodes a payload shorter than 56 bytes",
                ));
            }
            parse_rlp_list(input, payload_start, length, depth, item_count)
        }
    }
}

fn parse_rlp_list<'a>(
    input: &'a [u8],
    payload_start: usize,
    payload_length: usize,
    depth: usize,
    item_count: &mut usize,
) -> Result<(Rlp<'a>, usize), EthereumProofError> {
    let end = payload_start
        .checked_add(payload_length)
        .ok_or_else(rlp_length_overflow)?;
    let payload = input.get(payload_start..end).ok_or_else(rlp_truncated)?;
    let mut items = Vec::new();
    let mut offset = 0_usize;
    while offset < payload.len() {
        let (item, consumed) = parse_rlp(&payload[offset..], depth + 1, item_count)?;
        if consumed == 0 {
            return Err(EthereumProofError::new(
                "ethereum_rlp_truncated",
                "RLP parser made no progress",
            ));
        }
        offset = offset
            .checked_add(consumed)
            .ok_or_else(rlp_length_overflow)?;
        items.push(item);
    }
    Ok((Rlp::List(items), end))
}

fn decode_long_length(
    input: &[u8],
    length_of_length: u8,
) -> Result<(usize, usize), EthereumProofError> {
    let length_of_length = usize::from(length_of_length);
    if length_of_length == 0 || length_of_length > std::mem::size_of::<usize>() {
        return Err(EthereumProofError::new(
            "ethereum_rlp_length_invalid",
            "RLP length-of-length is unsupported",
        ));
    }
    let payload_start = 1_usize
        .checked_add(length_of_length)
        .ok_or_else(rlp_length_overflow)?;
    let length_bytes = input.get(1..payload_start).ok_or_else(rlp_truncated)?;
    if length_bytes[0] == 0 {
        return Err(EthereumProofError::new(
            "ethereum_rlp_noncanonical",
            "RLP long length has a leading zero",
        ));
    }
    let mut length = 0_usize;
    for byte in length_bytes {
        length = length
            .checked_mul(256)
            .and_then(|value| value.checked_add(usize::from(*byte)))
            .ok_or_else(rlp_length_overflow)?;
    }
    Ok((length, payload_start))
}

fn rlp_length_overflow() -> EthereumProofError {
    EthereumProofError::new("ethereum_rlp_length_overflow", "RLP length overflows usize")
}

fn rlp_truncated() -> EthereumProofError {
    EthereumProofError::new("ethereum_rlp_truncated", "RLP payload is truncated")
}

fn bytes_to_nibbles(bytes: &[u8]) -> Vec<u8> {
    let mut nibbles = Vec::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0f);
    }
    nibbles
}

fn rlp_encode_u64(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80];
    }
    let bytes = value.to_be_bytes();
    let first_nonzero = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len() - 1);
    let payload = &bytes[first_nonzero..];
    if payload.len() == 1 && payload[0] <= 0x7f {
        payload.to_vec()
    } else {
        let mut encoded = Vec::with_capacity(payload.len() + 1);
        // `payload` is a slice of a u64's eight-byte encoding, so this cast is
        // bounded by construction and cannot truncate.
        encoded.push(0x80 + payload.len() as u8);
        encoded.extend_from_slice(payload);
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_bytes(bytes: &[u8]) -> Vec<u8> {
        if bytes.len() == 1 && bytes[0] <= 0x7f {
            return bytes.to_vec();
        }
        if bytes.len() < 56 {
            let mut encoded = vec![0x80 + u8::try_from(bytes.len()).expect("short bytes")];
            encoded.extend_from_slice(bytes);
            return encoded;
        }
        let length = encode_length(bytes.len());
        let mut encoded = vec![0xb7 + u8::try_from(length.len()).expect("length of length")];
        encoded.extend_from_slice(&length);
        encoded.extend_from_slice(bytes);
        encoded
    }

    fn encode_list(items: &[Vec<u8>]) -> Vec<u8> {
        let payload = items.concat();
        if payload.len() < 56 {
            let mut encoded = vec![0xc0 + u8::try_from(payload.len()).expect("short list")];
            encoded.extend_from_slice(&payload);
            return encoded;
        }
        let length = encode_length(payload.len());
        let mut encoded = vec![0xf7 + u8::try_from(length.len()).expect("length of length")];
        encoded.extend_from_slice(&length);
        encoded.extend_from_slice(&payload);
        encoded
    }

    fn encode_length(length: usize) -> Vec<u8> {
        let bytes = length.to_be_bytes();
        bytes[bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(bytes.len() - 1)..]
            .to_vec()
    }

    fn receipt_with_topics(
        status: u8,
        emitter: [u8; 20],
        topics: &[[u8; 32]],
        data: &[u8],
    ) -> Vec<u8> {
        let topics = topics
            .iter()
            .map(|topic| encode_bytes(topic))
            .collect::<Vec<_>>();
        let log = encode_list(&[
            encode_bytes(&emitter),
            encode_list(&topics),
            encode_bytes(data),
        ]);
        encode_list(&[
            encode_bytes(&[status]),
            encode_bytes(&[1]),
            encode_bytes(&[0; ETHEREUM_LOG_BLOOM_BYTES]),
            encode_list(&[log]),
        ])
    }

    fn receipt(status: u8, emitter: [u8; 20], topic: [u8; 32], data: &[u8]) -> Vec<u8> {
        receipt_with_topics(status, emitter, &[topic], data)
    }

    fn abi_u64(value: u64) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn abi_address(value: [u8; 20]) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[12..].copy_from_slice(&value);
        word
    }

    fn abi_dynamic(value: &[u8]) -> Vec<u8> {
        let mut encoded = abi_u64(u64::try_from(value.len()).expect("test value length")).to_vec();
        encoded.extend_from_slice(value);
        encoded.resize(encoded.len().div_ceil(32) * 32, 0);
        encoded
    }

    fn single_receipt_proof(receipt_rlp: Vec<u8>) -> ([u8; 32], EthereumReceiptProofV1) {
        let leaf = encode_list(&[encode_bytes(&[0x20, 0x80]), encode_bytes(&receipt_rlp)]);
        let root: [u8; 32] = Keccak256::digest(&leaf).into();
        (
            root,
            EthereumReceiptProofV1 {
                transaction_index: 0,
                receipt_rlp,
                proof_nodes_rlp: vec![leaf],
            },
        )
    }

    #[test]
    fn verifies_real_receipt_trie_leaf_and_decodes_success_log() {
        let emitter = [0x11; 20];
        let topic = [0x22; 32];
        let data = vec![0x33; 96];
        let (root, proof) = single_receipt_proof(receipt(1, emitter, topic, &data));

        let verified = verify_ethereum_receipt_log(root, &proof, 0).expect("verified receipt");

        assert_eq!(verified.emitter, emitter);
        assert_eq!(verified.topics, vec![topic]);
        assert_eq!(verified.data, data);
    }

    #[test]
    fn rejects_wrong_root_key_value_status_log_index_and_noncanonical_rlp() {
        let emitter = [0x11; 20];
        let topic = [0x22; 32];
        let (root, proof) = single_receipt_proof(receipt(1, emitter, topic, &[0x33; 32]));

        let mut wrong_root = root;
        wrong_root[0] ^= 1;
        assert_eq!(
            verify_ethereum_receipt_log(wrong_root, &proof, 0)
                .expect_err("wrong root")
                .code(),
            "ethereum_receipt_proof_node_hash_mismatch"
        );

        let mut wrong_key = proof.clone();
        wrong_key.transaction_index = 1;
        assert_eq!(
            verify_ethereum_receipt_log(root, &wrong_key, 0)
                .expect_err("wrong transaction index")
                .code(),
            "ethereum_receipt_proof_path_mismatch"
        );

        let mut wrong_value = proof.clone();
        *wrong_value.receipt_rlp.last_mut().expect("receipt byte") ^= 1;
        assert_eq!(
            verify_ethereum_receipt_log(root, &wrong_value, 0)
                .expect_err("wrong receipt value")
                .code(),
            "ethereum_receipt_value_mismatch"
        );

        assert_eq!(
            verify_ethereum_receipt_log(root, &proof, 1)
                .expect_err("wrong log index")
                .code(),
            "ethereum_receipt_log_index_invalid"
        );

        let (failed_root, failed_proof) = single_receipt_proof(receipt(0, emitter, topic, &[]));
        assert_eq!(
            verify_ethereum_receipt_log(failed_root, &failed_proof, 0)
                .expect_err("failed receipt")
                .code(),
            "ethereum_receipt_not_successful"
        );

        assert_eq!(
            parse_rlp_exact(&[0x81, 0x01])
                .expect_err("noncanonical byte")
                .code(),
            "ethereum_rlp_noncanonical"
        );
    }

    #[test]
    fn binds_cancel_event_to_exact_controller_packet_and_deadline() {
        let controller = [0x11; 20];
        let packet_digest = [0x22; 32];
        let source_packet_commitment = [0x33; 32];
        let source_receipt_commitment = [0x44; 32];
        let signature: [u8; 32] =
            Keccak256::digest(b"PacketCancelled(bytes32,bytes32,bytes32,uint64,uint64)").into();
        let mut data = abi_u64(1_500).to_vec();
        data.extend_from_slice(&abi_u64(1_501));
        let receipt = receipt_with_topics(
            1,
            controller,
            &[
                signature,
                packet_digest,
                source_packet_commitment,
                source_receipt_commitment,
            ],
            &data,
        );
        let (root, proof) = single_receipt_proof(receipt);
        let log = verify_ethereum_receipt_log(root, &proof, 0).expect("cancel receipt proof");
        let expected = PacketCancelledEventV1 {
            controller,
            packet_digest,
            source_packet_commitment,
            source_receipt_commitment,
            deadline: 1_500,
            cancelled_at: 1_501,
        };

        verify_packet_cancelled_event(&log, &expected).expect("bound cancellation");

        let mut altered = expected.clone();
        altered.deadline += 1;
        assert_eq!(
            verify_packet_cancelled_event(&log, &altered)
                .expect_err("altered deadline")
                .code(),
            "ethereum_event_binding_mismatch"
        );
        let mut wrong_controller = expected;
        wrong_controller.controller[0] ^= 1;
        assert_eq!(
            verify_packet_cancelled_event(&log, &wrong_controller)
                .expect_err("wrong controller")
                .code(),
            "ethereum_event_emitter_mismatch"
        );
    }

    #[test]
    fn binds_dynamic_return_burn_event_and_rejects_noncanonical_tail() {
        let controller = [0x11; 20];
        let sender = [0x22; 20];
        let wrapped = [0x33; 20];
        let return_burn_id = [0x44; 32];
        let return_nonce = [0x55; 32];
        let recipient = b"pf124071fd53a12ca4556b7aa1f5ec98b585e73468";
        let native_asset = [0x66; 48];
        let signature: [u8; 32] = Keccak256::digest(
            b"ReturnBurned(bytes32,address,bytes32,string,bytes,uint256,uint256,address,address,uint256)",
        )
        .into();
        let recipient_tail = abi_dynamic(recipient);
        let native_tail = abi_dynamic(&native_asset);
        let mut data = abi_u64(7 * 32).to_vec();
        data.extend_from_slice(&abi_u64(
            u64::try_from(7 * 32 + recipient_tail.len()).expect("test offset"),
        ));
        data.extend_from_slice(&abi_u64(17));
        data.extend_from_slice(&abi_u64(1));
        data.extend_from_slice(&abi_address(controller));
        data.extend_from_slice(&abi_address(wrapped));
        data.extend_from_slice(&abi_u64(99));
        data.extend_from_slice(&recipient_tail);
        data.extend_from_slice(&native_tail);
        let receipt = receipt_with_topics(
            1,
            controller,
            &[signature, return_burn_id, abi_address(sender), return_nonce],
            &data,
        );
        let (root, proof) = single_receipt_proof(receipt);
        let log = verify_ethereum_receipt_log(root, &proof, 0).expect("return receipt proof");
        let expected = ReturnBurnedEventV1 {
            controller,
            return_burn_id,
            ethereum_sender: sender,
            return_nonce,
            pftl_recipient: String::from_utf8(recipient.to_vec()).expect("test recipient"),
            native_nav_asset_id: native_asset.to_vec(),
            amount_atoms: 17,
            ethereum_chain_id: 1,
            bridge_controller: controller,
            wrapped_navcoin: wrapped,
            burn_height: 99,
        };

        verify_return_burned_event(&log, &expected).expect("bound return burn");

        let mut wrong_sender = expected.clone();
        wrong_sender.ethereum_sender[0] ^= 1;
        assert_eq!(
            verify_return_burned_event(&log, &wrong_sender)
                .expect_err("wrong sender")
                .code(),
            "ethereum_event_binding_mismatch"
        );
        let mut wrong_token = expected.clone();
        wrong_token.wrapped_navcoin[0] ^= 1;
        assert_eq!(
            verify_return_burned_event(&log, &wrong_token)
                .expect_err("wrong token")
                .code(),
            "ethereum_event_binding_mismatch"
        );
        let mut wrong_nonce = expected.clone();
        wrong_nonce.return_nonce[0] ^= 1;
        assert_eq!(
            verify_return_burned_event(&log, &wrong_nonce)
                .expect_err("wrong nonce")
                .code(),
            "ethereum_event_binding_mismatch"
        );
        let mut wrong_recipient = expected.clone();
        wrong_recipient.pftl_recipient.push('x');
        assert_eq!(
            verify_return_burned_event(&log, &wrong_recipient)
                .expect_err("wrong recipient")
                .code(),
            "ethereum_event_binding_mismatch"
        );
        let mut wrong_amount = expected.clone();
        wrong_amount.amount_atoms += 1;
        assert_eq!(
            verify_return_burned_event(&log, &wrong_amount)
                .expect_err("wrong amount")
                .code(),
            "ethereum_event_binding_mismatch"
        );
        let mut wrong_topic = log.clone();
        wrong_topic.topics[0][0] ^= 1;
        assert_eq!(
            verify_return_burned_event(&wrong_topic, &expected)
                .expect_err("wrong event topic")
                .code(),
            "ethereum_event_topic_mismatch"
        );

        let mut noncanonical = log;
        noncanonical.data.push(0);
        assert_eq!(
            verify_return_burned_event(&noncanonical, &expected)
                .expect_err("trailing byte")
                .code(),
            "ethereum_event_abi_noncanonical"
        );
    }
}
