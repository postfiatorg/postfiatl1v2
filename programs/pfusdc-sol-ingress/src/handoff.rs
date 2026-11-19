//! Section 4.2. The light client pins the recorder, schedule, shred version,
//! trusted epoch/set and (for handoffs) the known-key commitment.
use crate::{
    certificate::{self, Aggregate, CertificateError, Finality, ValidatorSet},
    open_block_witness,
    recorder::{self, BlsKey, RecorderError},
    verify_delta, BlockError, BlockHeader, DepositError, Error, Reader,
};
use sha2::{Digest, Sha256};
use solana_epoch_schedule::{EpochSchedule, MINIMUM_SLOTS_PER_EPOCH};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub enum HandoffError {
    Input(DepositError),
    Block(BlockError),
    State(Error),
    Recorder(RecorderError),
    Certificate(CertificateError),
    InvalidSchedule,
    WrongBlockEpoch,
    NonCanonicalKeyTable,
    UnknownSignerKey,
    InsufficientOldStake,
}

/// Both proof types commit to these route and epoch bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecorderContext {
    pub recorder_program: [u8; 32],
    pub slot: u64,
    pub shred_version: u16,
    pub slots_per_epoch: u64,
    pub warmup: bool,
    pub epoch: u64,
    pub trusted_set_hash: [u8; 32],
}

impl RecorderContext {
    fn encode(&self, tag: &[u8; 8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend_from_slice(&self.recorder_program);
        out.extend_from_slice(&self.slot.to_le_bytes());
        out.extend_from_slice(&self.shred_version.to_le_bytes());
        out.extend_from_slice(&self.slots_per_epoch.to_le_bytes());
        out.push(u8::from(self.warmup));
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.extend_from_slice(&self.trusted_set_hash);
        out
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct KeyTablePublicValues {
    pub context: RecorderContext,
    pub keys_added: Vec<BlsKey>,
}

impl KeyTablePublicValues {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.context.encode(b"PFSOLKP1");
        out.extend_from_slice(&(self.keys_added.len() as u32).to_le_bytes());
        for key in &self.keys_added {
            out.extend_from_slice(key);
        }
        out
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HandoffPublicValues {
    pub context: RecorderContext,
    pub new_set_hash: [u8; 32],
    pub new_total_stake: u64,
    /// Sum of the stakes that can contribute to certificates in the new ranked set.
    pub listed_stake: u64,
    pub known_key_table_hash: [u8; 32],
}

impl HandoffPublicValues {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.context.encode(b"PFSOLHP2");
        out.extend_from_slice(&self.new_set_hash);
        out.extend_from_slice(&self.new_total_stake.to_le_bytes());
        out.extend_from_slice(&self.listed_stake.to_le_bytes());
        out.extend_from_slice(&self.known_key_table_hash);
        out
    }
}

struct ProofInput<'a> {
    trusted_epoch: u64,
    schedule: EpochSchedule,
    trusted: ValidatorSet,
    rest: Reader<'a>,
}

impl<'a> ProofInput<'a> {
    fn read(bytes: &'a [u8]) -> Result<Self, HandoffError> {
        let mut rest = Reader { bytes, at: 0 };
        let truncated = |_| HandoffError::Input(DepositError::Truncated);
        let trusted_epoch = rest.u64().map_err(truncated)?;
        let slots = rest.u64().map_err(truncated)?;
        let warmup = match rest.u8().map_err(truncated)? {
            0 => false,
            1 => true,
            _ => return Err(HandoffError::InvalidSchedule),
        };
        if slots < MINIMUM_SLOTS_PER_EPOCH || slots.checked_next_power_of_two().is_none() {
            return Err(HandoffError::InvalidSchedule);
        }
        let schedule = EpochSchedule::custom(slots, slots, warmup);
        let trusted = rest.validator_set().map_err(HandoffError::Input)?;
        Ok(Self {
            trusted_epoch,
            schedule,
            trusted,
            rest,
        })
    }

    fn block(
        &mut self,
        epoch: u64,
    ) -> Result<(Finality<'a>, BlockHeader, RecorderContext, &'a [u8]), HandoffError> {
        let finality = self.rest.finality().map_err(HandoffError::Input)?;
        let (block, state) =
            open_block_witness(&self.rest.bytes[self.rest.at..]).map_err(HandoffError::Block)?;
        if self.schedule.get_epoch(block.slot) != epoch {
            return Err(HandoffError::WrongBlockEpoch);
        }
        let mut reader = Reader {
            bytes: state,
            at: 0,
        };
        let bank = reader.array().map_err(HandoffError::State)?;
        let program = reader.array().map_err(HandoffError::State)?;
        let target: [u8; 32] = reader.array().map_err(HandoffError::State)?;
        let mut data = None;
        verify_delta(&mut reader, bank, |address, old, new| {
            if address == target {
                // Identical states cancel out of the lattice delta and prove nothing.
                // For a real change (creation or update), the distinct-address delta
                // authenticates the new state. Only its pinned owner can write the
                // recorder data. The parser checks the kind and bank epoch, and the
                // stake header binds all batches to that epoch's single runtime total.
                if old == new || new.lamports == 0 || new.executable || new.owner != program {
                    return Err(Error::InvalidRecorderAccount);
                }
                data = Some(new.data);
            }
            Ok(())
        })
        .map_err(HandoffError::State)?;
        let data = data.ok_or(HandoffError::State(Error::InvalidRecorderAccount))?;
        let context = RecorderContext {
            recorder_program: program,
            epoch,
            slot: block.slot,
            shred_version: block.shred_version,
            slots_per_epoch: self.schedule.slots_per_epoch,
            warmup: self.schedule.warmup,
            trusted_set_hash: self.trusted.hash(),
        };
        Ok((finality, block, context, data))
    }
}

pub fn verify_key_table_witness(witness: &[u8]) -> Result<KeyTablePublicValues, HandoffError> {
    let mut input = ProofInput::read(witness)?;
    let (finality, block, context, data) = input.block(input.trusted_epoch)?;
    certificate::verify_finality(
        &input.trusted,
        block.slot,
        &block.block_id,
        block.shred_version,
        &finality,
    )
    .map_err(HandoffError::Certificate)?;
    let keys_added =
        recorder::recorded_keys(data, context.epoch).map_err(HandoffError::Recorder)?;
    Ok(KeyTablePublicValues {
        context,
        keys_added,
    })
}

/// The table is a set of compressed keys, in strictly increasing byte order.
pub fn known_key_table_hash(keys: &[BlsKey]) -> Result<[u8; 32], HandoffError> {
    let count = u32::try_from(keys.len()).map_err(|_| HandoffError::NonCanonicalKeyTable)?;
    if keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(HandoffError::NonCanonicalKeyTable);
    }
    let mut hash = Sha256::new()
        .chain_update(b"postfiat.pfusdc.solana.known_keys.v1")
        .chain_update(count.to_le_bytes());
    for key in keys {
        hash.update(key);
    }
    Ok(hash.finalize().into())
}

fn authenticated_ranks(
    set: &ValidatorSet,
    aggregate: &Aggregate,
    table: &[BlsKey],
) -> Result<Vec<usize>, HandoffError> {
    let ranks = certificate::signer_ranks(aggregate.bitmap, set.validators.len())
        .map_err(HandoffError::Certificate)?;
    if ranks
        .iter()
        .any(|&rank| table.binary_search(&set.validators[rank].pubkey).is_err())
    {
        return Err(HandoffError::UnknownSignerKey);
    }
    Ok(ranks)
}

pub fn verify_handoff_witness(witness: &[u8]) -> Result<HandoffPublicValues, HandoffError> {
    let mut input = ProofInput::read(witness)?;
    let truncated = |_| HandoffError::Input(DepositError::Truncated);
    let count = input.rest.u32().map_err(truncated)? as usize;
    let len = count
        .checked_mul(certificate::PUBKEY_LEN)
        .ok_or(HandoffError::Input(DepositError::Truncated))?;
    let table: Vec<BlsKey> = input
        .rest
        .take(len)
        .map_err(truncated)?
        .chunks_exact(certificate::PUBKEY_LEN)
        .map(|row| row.try_into().unwrap())
        .collect();
    let known_key_table_hash = known_key_table_hash(&table)?;
    let epoch = input
        .trusted_epoch
        .checked_add(1)
        .ok_or(HandoffError::WrongBlockEpoch)?;
    let (finality, block, context, data) = input.block(epoch)?;
    let new_set = recorder::recorded_set(data, epoch).map_err(HandoffError::Recorder)?;
    let ranks = match &finality {
        Finality::Fast(cert) => authenticated_ranks(&new_set, cert, &table)?,
        Finality::Slow { notarize, finalize } => {
            // Finalize names only the slot. Count old validators that also signed
            // the notarization for this specific block. Check ALL keys of both bitmaps.
            let notarizers = authenticated_ranks(&new_set, notarize, &table)?;
            let mut finalizers = authenticated_ranks(&new_set, finalize, &table)?;
            finalizers.retain(|rank| notarizers.binary_search(rank).is_ok());
            finalizers
        }
    };
    let old: BTreeMap<_, _> = input
        .trusted
        .validators
        .iter()
        .map(|v| (v.pubkey, v.stake))
        .collect();
    let old_stake: u128 = ranks
        .iter()
        .filter_map(|&rank| old.get(&new_set.validators[rank].pubkey))
        .map(|&stake| u128::from(stake))
        .sum();
    if old_stake < input.trusted.total_stake - input.trusted.total_stake / 3 {
        return Err(HandoffError::InsufficientOldStake);
    }
    certificate::verify_finality(
        &new_set,
        block.slot,
        &block.block_id,
        block.shred_version,
        &finality,
    )
    .map_err(HandoffError::Certificate)?;
    Ok(HandoffPublicValues {
        context,
        new_set_hash: new_set.hash(),
        new_total_stake: new_set.total_stake as u64,
        // Some votes can be absent or ineligible. The full runtime stake remains
        // the certificate denominator. Missing weight can prevent finality.
        // recorded_set validated the sum against the u64 runtime total.
        listed_stake: new_set.validators.iter().map(|v| v.stake).sum(),
        known_key_table_hash,
    })
}
