//! A leaf proves one run of account records whose addresses strictly increase. Its journal
//! carries the run's first and last address, its record count, its 1024-lane signed delta
//! and each receipt claim it met. The parent sees no account record: it checks that the runs
//! are in address order (so no address repeats), that their counts and deltas turn the sum
//! before the block into the sum after it, and performs every bank, shred and certificate check.
use super::*;

pub(crate) trait Delta {
    /// Applies the block's `count` distinct changed accounts to `running` and collects its receipts.
    fn apply(&self, reader: &mut Reader, count: u32, running: &mut LtHash,
        vault_program: &[u8; 32], route_binding: &[u8; 32], receipts: &mut Vec<Receipt>) -> Result<(), Error>;
}

/// The records follow in the witness.
impl Delta for () {
    fn apply(&self, reader: &mut Reader, count: u32, running: &mut LtHash,
        vault_program: &[u8; 32], route_binding: &[u8; 32], receipts: &mut Vec<Receipt>) -> Result<(), Error> {
        walk_records(reader, count, running, |address, old, new| {
            if let Some(data) = receipt_claim(vault_program, &old, &new)? {
                receipts.push(receipt(vault_program, route_binding, address, data)?);
            }
            Ok(())
        })
    }
}

pub const MAX_SHARDS: usize = 32;
/// vault program | route binding | first address | last address | count u32 | delta | receipt count u32
pub const JOURNAL_FIXED: usize = 4 * 32 + 4 + LT_BYTES + 4;
const CLAIM_LEN: usize = 32 + RECEIPT_LEN;

/// Native and guest leaf entry point. Input: `vault program | route binding | records`.
/// Journal: the fixed part above, then each receipt claim as `address | receipt data`.
pub fn leaf(input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut reader = Reader { bytes: input, at: 0 };
    let vault_program: [u8; 32] = reader.array()?;
    let route_binding: [u8; 32] = reader.array()?;
    if reader.at == input.len() { return Err(Error::Truncated); }
    let mut sum = LtHash([0; LT_ELEMENTS]);
    let (mut first, mut last, mut count) = ([0u8; 32], [0u8; 32], 0u32);
    let mut claims = Vec::new();
    while reader.at < input.len() {
        let address: [u8; 32] = reader.array()?;
        let old = reader.account()?;
        let new = reader.account()?;
        if count == 0 { first = address; } else if address <= last { return Err(Error::UnsortedAccounts); }
        last = address;
        count = count.checked_add(1).ok_or(Error::Truncated)?;
        apply_account_change(&mut sum, &address, &old, &new);
        if let Some(data) = receipt_claim(&vault_program, &old, &new)? {
            // Refuse a malformed claim here: the parent decodes only well-formed ones.
            receipt(&vault_program, &route_binding, address, data)?;
            claims.extend_from_slice(&address);
            claims.extend_from_slice(data);
        }
    }
    let mut journal = Vec::with_capacity(JOURNAL_FIXED + claims.len());
    journal.extend_from_slice(&vault_program);
    journal.extend_from_slice(&route_binding);
    journal.extend_from_slice(&first);
    journal.extend_from_slice(&last);
    journal.extend_from_slice(&count.to_le_bytes());
    journal.extend_from_slice(&sum.to_bytes());
    journal.extend_from_slice(&((claims.len() / CLAIM_LEN) as u32).to_le_bytes());
    journal.extend_from_slice(&claims);
    Ok(journal)
}

/// Each frame is journal length (u32 LE), then the verified leaf journal.
pub(crate) struct ProvenRanges<'a>(pub(crate) &'a [u8]);
impl Delta for ProvenRanges<'_> {
    fn apply(&self, _: &mut Reader, count: u32, running: &mut LtHash,
        vault_program: &[u8; 32], route_binding: &[u8; 32], receipts: &mut Vec<Receipt>) -> Result<(), Error> {
        let mut covered = 0u64;
        let mut previous: Option<[u8; 32]> = None;
        for journal in frames(self.0)? {
            let mut reader = Reader { bytes: journal, at: 0 };
            if reader.array::<32>()? != *vault_program || reader.array::<32>()? != *route_binding {
                return Err(Error::DeltaMismatch);
            }
            let first: [u8; 32] = reader.array()?;
            let last: [u8; 32] = reader.array()?;
            let records = reader.u32()?;
            // Runs in strictly increasing address order share no address.
            if records == 0 || first > last || previous.is_some_and(|p| first <= p) {
                return Err(Error::UnsortedAccounts);
            }
            previous = Some(last);
            covered += u64::from(records);
            running.add(reader.take(LT_BYTES)?.try_into().expect("fixed length"));
            for _ in 0..reader.u32()? {
                let address = reader.array()?;
                receipts.push(receipt(vault_program, route_binding, address, reader.take(RECEIPT_LEN)?)?);
            }
            if reader.at != journal.len() { return Err(Error::DeltaMismatch); }
        }
        if covered != u64::from(count) { return Err(Error::DeltaMismatch); }
        Ok(())
    }
}

pub fn frames(bytes: &[u8]) -> Result<Vec<&[u8]>, Error> {
    let mut reader = Reader { bytes, at: 0 };
    let mut out = Vec::new();
    while reader.at < bytes.len() {
        if out.len() == MAX_SHARDS { return Err(Error::DeltaMismatch); }
        let len = reader.u32()? as usize;
        if len < JOURNAL_FIXED { return Err(Error::DeltaMismatch); }
        out.push(reader.take(len)?);
    }
    if out.is_empty() { return Err(Error::DeltaMismatch); }
    Ok(out)
}

/// The caller must authenticate every journal against the fixed leaf program.
/// Only the recursive guest calls this to produce an admission journal.
/// `header` is the deposit witness up to and including the changed-account count.
pub fn aggregate(header: &[u8], authenticated_frames: &[u8]) -> Result<DepositPublicValues, DepositError> {
    verify_deposit(header, true, &ProvenRanges(authenticated_frames))
}

/// Host side: the deposit witness with its records in address order, as the leaves require.
/// The statement is unchanged; receipts are then listed in address order by every verifier.
pub fn canonical(witness: &[u8]) -> Result<Vec<u8>, DepositError> {
    let records = *partition(witness, 1)?.first().ok_or(DepositError::Truncated)?;
    let start = witness.len() - records.len();
    let mut reader = Reader { bytes: records, at: 0 };
    let mut spans = Vec::new();
    let mut walk = || -> Result<(), Error> {
        while reader.at < records.len() {
            let begin = reader.at;
            let address: [u8; 32] = reader.array()?;
            reader.account()?;
            reader.account()?;
            spans.push((address, begin..reader.at));
        }
        Ok(())
    };
    walk().map_err(|e| DepositError::Block(BlockError::State(e)))?;
    spans.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = witness[..start].to_vec();
    for (_, span) in spans { out.extend_from_slice(&records[span]); }
    Ok(out)
}

/// Host side: the aggregate's input and each leaf's input for a canonical witness.
pub fn inputs(witness: &[u8], workers: usize) -> Result<(&[u8], Vec<Vec<u8>>), DepositError> {
    let ranges = partition(witness, workers)?;
    let records: usize = ranges.iter().map(|r| r.len()).sum();
    let header = &witness[..witness.len() - records];
    // The state witness opens with bank hash | vault program | route binding; the count ends it.
    let state = header.len() - (96 + 2 * (72 + LT_BYTES) + 4);
    let binding = &header[state + 32..state + 96];
    Ok((header, ranges.into_iter().map(|r| [binding, r].concat()).collect()))
}

/// Partition the validated record stream in witness order. The byte weight also
/// charges each account for its two 2048-byte XOF outputs.
pub fn partition(witness: &[u8], workers: usize) -> Result<Vec<&[u8]>, DepositError> {
    if !(1..=MAX_SHARDS).contains(&workers) { return Err(DepositError::Truncated); }
    let mut reader = Reader { bytes: witness, at: 0 };
    reader.validator_set()?;
    reader.deposit_finality()?;
    let (_, state) = open_block_witness(&witness[reader.at..]).map_err(DepositError::Block)?;
    let mut reader = Reader { bytes: state, at: 0 };
    let mut ranges = Vec::new();
    let mut split = || -> Result<(), Error> {
        reader.take(96 + 2 * (72 + LT_BYTES))?;
        let count = reader.u32()? as usize;
        let mut start = reader.at;
        let mut weight = 0;
        let total = state.len() - start + count * (2 * LT_BYTES);
        for i in 0..count {
            let begin = reader.at;
            reader.array::<32>()?;
            reader.account()?;
            reader.account()?;
            weight += reader.at - begin + 2 * LT_BYTES;
            if i + 1 == count || (ranges.len() + 1 < workers && weight >= total.div_ceil(workers) * (ranges.len() + 1)) {
                ranges.push(&state[start..reader.at]);
                start = reader.at;
            }
        }
        if reader.at != state.len() { return Err(Error::TrailingBytes); }
        Ok(())
    };
    split().map_err(|e| DepositError::Block(BlockError::State(e)))?;
    Ok(ranges)
}

