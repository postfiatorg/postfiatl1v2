//! Step 1 of the deposit proof: raw data shreds to the block ID.
//!
//! Each data shred carries its own Merkle proof up to the root of its erasure set, so every
//! shred is checked on its own with the same fold Agave uses (`ledger/src/shred/merkle_tree.rs`,
//! `get_merkle_root`). The block ID is the Merkle root over the erasure-set roots plus one
//! leaf for the parent (`ledger/src/blockstore.rs`, `build_double_merkle_tree`).
//!
//! Only the shape mainnet produces today is accepted: chained Merkle data shreds in erasure
//! sets of 32 data and 32 coding shreds. Anything else is refused, not interpreted.

use sha2::{Digest, Sha256};

pub const SHRED_PAYLOAD: usize = 1203;
pub const DATA_SHREDS_PER_SET: usize = 32;
const PROOF_ENTRIES: usize = 6; // a tree of 64 leaves
const NODE: usize = 20; // Merkle nodes are truncated to 20 bytes when joined
const SIGNATURE: usize = 64;
const HEADERS: usize = 88; // signature 64 | variant 1 | slot 8 | index 4 | version 2 | fec_set_index 4 | parent_offset 2 | flags 1 | size 2
const LEAF_PREFIX: &[u8] = b"\x00SOLANA_MERKLE_SHREDS_LEAF";
const NODE_PREFIX: &[u8] = b"\x01SOLANA_MERKLE_SHREDS_NODE";
const LAST_IN_SLOT: u8 = 0b1100_0000;
const DATA_COMPLETE: u8 = 0b0100_0000;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ShredError {
    Empty,
    IncompleteErasureSet,
    WrongPayloadSize,
    UnsupportedVariant,
    WrongSlot,
    WrongIndex,
    WrongErasureSet,
    MixedShredVersion,
    LastInSlotFlag,
    BadDataSize,
    ErasureSetRootMismatch,
    /// The block's data does not end on a component boundary.
    UnterminatedComponent,
    EmptyComponent,
    MalformedMarker,
    NoFooter,
    MultipleFooters,
    UnsupportedFooterVersion,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Block {
    pub block_id: [u8; 32],
    pub shred_version: u16,
    pub erasure_sets: u32,
    /// The data sections of the shreds in order: the serialized block components.
    pub stream: Vec<u8>,
    /// End offset in `stream` of each block component. Agave deserializes one component per
    /// run of shreds ending in a shred flagged DATA_COMPLETE (`ledger/src/blockstore.rs`,
    /// completed ranges), so in a block validators accepted these are the component boundaries.
    pub component_ends: Vec<usize>,
}

fn join(left: &[u8], right: &[u8]) -> [u8; 32] {
    Sha256::new()
        .chain_update(NODE_PREFIX)
        .chain_update(&left[..NODE])
        .chain_update(&right[..NODE])
        .finalize()
        .into()
}

/// Root of a tree built the way Agave's `MerkleTree::try_new_with_len` builds it: an odd
/// node is paired with itself, and a single leaf is its own root.
fn tree_root(mut level: Vec<[u8; 32]>) -> [u8; 32] {
    while level.len() > 1 {
        let size = level.len();
        level = (0..size)
            .step_by(2)
            .map(|i| join(&level[i], &level[(i + 1).min(size - 1)]))
            .collect();
    }
    level[0]
}

pub fn parent_leaf(parent_slot: u64, parent_block_id: &[u8; 32], erasure_sets: u32) -> [u8; 32] {
    Sha256::new()
        .chain_update(parent_slot.to_le_bytes())
        .chain_update(parent_block_id)
        .chain_update(erasure_sets.to_le_bytes())
        .finalize()
        .into()
}

/// The erasure-set root one data shred commits to, with its data section and shred version.
fn check_shred<'a>(
    payload: &'a [u8],
    slot: u64,
    index: usize,
    last: bool,
) -> Result<([u8; 32], &'a [u8], u16, bool), ShredError> {
    if payload.len() != SHRED_PAYLOAD {
        return Err(ShredError::WrongPayloadSize);
    }
    let variant = payload[SIGNATURE];
    let resigned = match variant & 0xF0 {
        0x90 => false,
        0xB0 => true,
        _ => return Err(ShredError::UnsupportedVariant),
    };
    if usize::from(variant & 0x0F) != PROOF_ENTRIES {
        return Err(ShredError::UnsupportedVariant);
    }
    if u64::from_le_bytes(payload[65..73].try_into().expect("fixed")) != slot {
        return Err(ShredError::WrongSlot);
    }
    if u32::from_le_bytes(payload[73..77].try_into().expect("fixed")) as usize != index {
        return Err(ShredError::WrongIndex);
    }
    let version = u16::from_le_bytes(payload[77..79].try_into().expect("fixed"));
    let set_start = index / DATA_SHREDS_PER_SET * DATA_SHREDS_PER_SET;
    if u32::from_le_bytes(payload[79..83].try_into().expect("fixed")) as usize != set_start {
        return Err(ShredError::WrongErasureSet);
    }
    if (payload[85] & LAST_IN_SLOT == LAST_IN_SLOT) != last {
        return Err(ShredError::LastInSlotFlag);
    }

    let proof_at = SHRED_PAYLOAD - PROOF_ENTRIES * NODE - if resigned { SIGNATURE } else { 0 };
    let data_end = proof_at - 32; // the chained root sits between the data and the proof
    let size = usize::from(u16::from_le_bytes(
        payload[86..88].try_into().expect("fixed"),
    ));
    if size < HEADERS || size > data_end {
        return Err(ShredError::BadDataSize);
    }

    let mut node: [u8; 32] = Sha256::new()
        .chain_update(LEAF_PREFIX)
        .chain_update(&payload[SIGNATURE..proof_at])
        .finalize()
        .into();
    let mut position = index - set_start;
    for entry in payload[proof_at..proof_at + PROOF_ENTRIES * NODE].chunks_exact(NODE) {
        node = if position % 2 == 0 {
            join(&node, entry)
        } else {
            join(entry, &node)
        };
        position >>= 1;
    }
    Ok((
        node,
        &payload[HEADERS..size],
        version,
        payload[85] & DATA_COMPLETE != 0,
    ))
}

/// The erasure-set root a single data shred commits to. Exposed so each shred can be compared
/// with Agave's own reading of it.
pub fn erasure_set_root(slot: u64, index: usize, payload: &[u8]) -> Result<[u8; 32], ShredError> {
    if payload.len() != SHRED_PAYLOAD {
        return Err(ShredError::WrongPayloadSize);
    }
    let last = payload[85] & LAST_IN_SLOT == LAST_IN_SLOT;
    check_shred(payload, slot, index, last).map(|(root, _, _, _)| root)
}

/// `shreds` are the block's data shred payloads in index order, starting at index 0.
pub fn block_from_shreds(
    slot: u64,
    parent_slot: u64,
    parent_block_id: &[u8; 32],
    shreds: &[&[u8]],
) -> Result<Block, ShredError> {
    if shreds.is_empty() {
        return Err(ShredError::Empty);
    }
    if shreds.len() % DATA_SHREDS_PER_SET != 0 {
        return Err(ShredError::IncompleteErasureSet);
    }
    let mut roots: Vec<[u8; 32]> = Vec::with_capacity(shreds.len() / DATA_SHREDS_PER_SET + 1);
    let mut stream = Vec::new();
    let mut component_ends: Vec<usize> = Vec::new();
    let mut shred_version = None;
    for (index, payload) in shreds.iter().enumerate() {
        let (root, data, version, complete) =
            check_shred(payload, slot, index, index == shreds.len() - 1)?;
        if *shred_version.get_or_insert(version) != version {
            return Err(ShredError::MixedShredVersion);
        }
        if index % DATA_SHREDS_PER_SET == 0 {
            roots.push(root);
        } else if roots.last() != Some(&root) {
            return Err(ShredError::ErasureSetRootMismatch);
        }
        stream.extend_from_slice(data);
        if complete {
            if component_ends.last().copied().unwrap_or(0) == stream.len() {
                return Err(ShredError::EmptyComponent);
            }
            component_ends.push(stream.len());
        }
    }
    if component_ends.last() != Some(&stream.len()) {
        return Err(ShredError::UnterminatedComponent);
    }
    let erasure_sets = roots.len() as u32;
    roots.push(parent_leaf(parent_slot, parent_block_id, erasure_sets));
    Ok(Block {
        block_id: tree_root(roots),
        shred_version: shred_version.expect("non-empty"),
        erasure_sets,
        stream,
        component_ends,
    })
}

const MARKER_VERSION_V1: u16 = 1;
const MARKER_TAG_FOOTER: u8 = 0;
const FOOTER_VERSION_V1: u8 = 1;

/// Step 2: the bank hash from the block's footer.
///
/// A component whose first eight bytes are zero is a block marker; an entry batch never has a
/// zero count (`entry/src/block_component.rs`). Agave's replay refuses a block with no footer
/// or more than one (`runtime/src/block_component_processor.rs`, `MissingBlockFooter`,
/// `MultipleBlockFooters`), so a block validators finalized has exactly one. This function
/// requires exactly one as well, and reads only marker components, so no transaction is parsed.
pub fn footer_bank_hash(block: &Block) -> Result<[u8; 32], ShredError> {
    let mut found = None;
    let mut start = 0;
    for &end in &block.component_ends {
        let component = &block.stream[start..end];
        start = end;
        if component.len() < 8 || component[..8] != [0u8; 8] {
            continue;
        }
        // marker version u16 | tag u8 | body length u16 | body
        if component.len() < 13 {
            return Err(ShredError::MalformedMarker);
        }
        if u16::from_le_bytes([component[8], component[9]]) != MARKER_VERSION_V1 {
            return Err(ShredError::MalformedMarker);
        }
        let tag = component[10];
        let length = usize::from(u16::from_le_bytes([component[11], component[12]]));
        if component.len() != 13 + length {
            return Err(ShredError::MalformedMarker);
        }
        if tag != MARKER_TAG_FOOTER {
            continue;
        }
        let body = &component[13..];
        if body.len() < 33 {
            return Err(ShredError::MalformedMarker);
        }
        if body[0] != FOOTER_VERSION_V1 {
            return Err(ShredError::UnsupportedFooterVersion);
        }
        if found.is_some() {
            return Err(ShredError::MultipleFooters);
        }
        found = Some(body[1..33].try_into().expect("length checked"));
    }
    found.ok_or(ShredError::NoFooter)
}
