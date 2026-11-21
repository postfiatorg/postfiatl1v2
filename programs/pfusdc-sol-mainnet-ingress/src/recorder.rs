//! Recorder account ABI v1. All integers are little endian. See the byte tables in README.md.
use crate::certificate::{CertificateError, Validator, ValidatorSet, PUBKEY_LEN};
use bls12_381::G1Affine;
use std::collections::{BTreeMap, BTreeSet};

pub const KEY_MAGIC: &[u8; 8] = b"PFSOLKY1";
pub const STAKE_MAGIC: &[u8; 8] = b"PFSOLST1";
pub const EPOCH_OFFSET: usize = 8;
pub const KEY_COUNT_OFFSET: usize = 16;
pub const KEY_HEADER_LEN: usize = 20;
pub const KEY_ROW_LEN: usize = PUBKEY_LEN;
pub const TOTAL_STAKE_OFFSET: usize = 16;
pub const STAKE_COUNT_OFFSET: usize = 24;
pub const STAKE_HEADER_LEN: usize = 28;
pub const VOTE_OFFSET: usize = 0;
pub const NODE_OFFSET: usize = 32;
pub const STAKE_OFFSET: usize = 64;
pub const BLS_OFFSET: usize = 72;
pub const STAKE_ROW_LEN: usize = BLS_OFFSET + PUBKEY_LEN;
pub type BlsKey = [u8; PUBKEY_LEN];

#[derive(Debug, PartialEq, Eq)]
pub enum RecorderError {
    Malformed,
    WrongEpoch,
    InvalidKey,
    DuplicateVoteAccount,
    InvalidTotalStake,
    Set(CertificateError),
}

fn valid_key(key: &BlsKey) -> bool {
    Option::<G1Affine>::from(G1Affine::from_compressed(key))
        .is_some_and(|point| !bool::from(point.is_identity()))
}

fn rows<'a>(
    data: &'a [u8],
    magic: &[u8; 8],
    epoch: u64,
    header: usize,
    count_at: usize,
    width: usize,
) -> Result<&'a [u8], RecorderError> {
    if data.len() < header || &data[..8] != magic {
        return Err(RecorderError::Malformed);
    }
    if data[EPOCH_OFFSET..EPOCH_OFFSET + 8] != epoch.to_le_bytes() {
        return Err(RecorderError::WrongEpoch);
    }
    let count = u32::from_le_bytes(data[count_at..count_at + 4].try_into().unwrap()) as usize;
    let payload = &data[header..];
    if count == 0 || count.checked_mul(width) != Some(payload.len()) {
        return Err(RecorderError::Malformed);
    }
    Ok(payload)
}

pub fn recorded_keys(data: &[u8], epoch: u64) -> Result<Vec<BlsKey>, RecorderError> {
    let payload = rows(
        data,
        KEY_MAGIC,
        epoch,
        KEY_HEADER_LEN,
        KEY_COUNT_OFFSET,
        KEY_ROW_LEN,
    )?;
    let mut keys = BTreeSet::new();
    for row in payload.chunks_exact(KEY_ROW_LEN) {
        let key = row.try_into().unwrap();
        if !valid_key(&key) {
            return Err(RecorderError::InvalidKey);
        }
        keys.insert(key);
    }
    Ok(keys.into_iter().collect())
}

/// Mirrors Agave 141da0c3 runtime/src/epoch_stakes.rs, BLSPubkeyToRankMap::new.
/// Agave's input is a map indexed by vote account address. A repeated address here
/// cannot represent two entries of that map.
pub fn recorded_set(data: &[u8], epoch: u64) -> Result<ValidatorSet, RecorderError> {
    let payload = rows(
        data,
        STAKE_MAGIC,
        epoch,
        STAKE_HEADER_LEN,
        STAKE_COUNT_OFFSET,
        STAKE_ROW_LEN,
    )?;
    let total = u64::from_le_bytes(
        data[TOTAL_STAKE_OFFSET..TOTAL_STAKE_OFFSET + 8]
            .try_into()
            .unwrap(),
    );
    let mut votes = BTreeSet::new();
    let mut keys = BTreeMap::<BlsKey, usize>::new();
    let mut nodes = BTreeMap::<[u8; 32], usize>::new();
    let mut candidates = Vec::new();
    let mut listed = 0u128;
    for row in payload.chunks_exact(STAKE_ROW_LEN) {
        if !votes.insert(&row[VOTE_OFFSET..NODE_OFFSET]) {
            return Err(RecorderError::DuplicateVoteAccount);
        }
        let node: [u8; 32] = row[NODE_OFFSET..STAKE_OFFSET].try_into().unwrap();
        let stake = u64::from_le_bytes(row[STAKE_OFFSET..BLS_OFFSET].try_into().unwrap());
        listed += u128::from(stake);
        let pubkey = row[BLS_OFFSET..].try_into().unwrap();
        if stake == 0 || !valid_key(&pubkey) {
            continue;
        }
        *keys.entry(pubkey).or_default() += 1;
        *nodes.entry(node).or_default() += 1;
        candidates.push((node, Validator { pubkey, stake }));
    }
    if listed > u128::from(total) {
        return Err(RecorderError::InvalidTotalStake);
    }
    let mut validators: Vec<_> = candidates
        .into_iter()
        .filter_map(|(node, v)| (keys[&v.pubkey] == 1 && nodes[&node] == 1).then_some(v))
        .collect();
    validators.sort_unstable_by(|a, b| b.stake.cmp(&a.stake).then(a.pubkey.cmp(&b.pubkey)));
    ValidatorSet::new(validators, u128::from(total)).map_err(RecorderError::Set)
}
