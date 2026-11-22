//! Open the parent commitment of each certified descendant, from tip to deposit.
use super::{DepositError, Reader, shreds};

pub const MAX_LINKS: usize = 256;
// Agave caps a slot at 32,768 data shreds, with 32 per FEC set.
pub const MAX_FEC_SETS: usize = 1024;

pub(crate) struct Path {
    pub tip: (u64, [u8; 32]),
    pub ancestor: (u64, [u8; 32]),
}

impl Path {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self, DepositError> {
        let invalid = |_| DepositError::InvalidAncestry;
        let tip_slot = reader.u64().map_err(invalid)?;
        let length = reader.u32().map_err(invalid)? as usize;
        if !(1..=MAX_LINKS).contains(&length) {
            return Err(DepositError::InvalidAncestry);
        }
        let mut tip_id = [0; 32];
        let mut slot = tip_slot;
        let mut expected = None;
        for _ in 0..length {
            let parent_slot = reader.u64().map_err(invalid)?;
            let parent_id = reader.array().map_err(invalid)?;
            let sets = reader.u32().map_err(invalid)? as usize;
            if parent_slot >= slot || !(1..=MAX_FEC_SETS).contains(&sets) {
                return Err(DepositError::InvalidAncestry);
            }
            let mut leaves = Vec::with_capacity(sets + 1);
            for _ in 0..sets {
                leaves.push(reader.array().map_err(invalid)?);
            }
            leaves.push(shreds::parent_leaf(parent_slot, &parent_id, sets as u32));
            let root = shreds::tree_root(leaves);
            if let Some(child_parent) = expected {
                if root != child_parent { return Err(DepositError::InvalidAncestry); }
            } else {
                tip_id = root;
            }
            slot = parent_slot;
            expected = Some(parent_id);
        }
        Ok(Self { tip: (tip_slot, tip_id), ancestor: (slot, expected.unwrap()) })
    }
}
