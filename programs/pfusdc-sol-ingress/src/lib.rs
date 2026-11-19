//! pfUSDC Solana ingress: the state layer of the deposit proof.
//!
//! Specification: `docs/specs/pfusdc-solana-trustless-bridge-spec-20261118.md`, section 4.1.
//! This file implements steps 3 to 5:
//!
//! 3. The bank hash opens to the account lattice sum after the block, and the parent bank
//!    hash inside that preimage opens to the sum before it.
//! 4. Subtracting each changed account's old hash and adding its new one, with no address
//!    repeated, turns the sum before into the sum after.
//! 5. Receipts are the changed accounts that did not exist before the block, are owned by
//!    the pinned vault program, and carry the receipt layout with the pinned route binding.
//!
//! Steps 1, 2 (shreds to block ID to bank hash) and 6 (the finalization certificate) are not
//! here yet. Until they are, the bank hash is an input and nothing this file returns is
//! evidence of a deposit on its own.
//!
//! Formats follow Agave at commit `141da0c3`:
//! - account hash input: `accounts-db/src/accounts_db.rs`, `write_account_hash_input`;
//! - lattice hash: `lattice-hash/src/lt_hash.rs`;
//! - bank hash: `runtime/src/bank.rs`, `hash_internal_state` (no hard-fork step).

use sha2::{Digest, Sha256};

pub mod certificate;
pub mod recorder;
pub mod handoff;
pub use handoff::{verify_handoff_witness, verify_key_table_witness};
pub mod shreds;

pub const LT_ELEMENTS: usize = 1024;
pub const LT_BYTES: usize = 2 * LT_ELEMENTS;
pub const RECEIPT_MAGIC: &[u8; 8] = b"PFSOLRC1";
/// magic | route_binding | pftl_recipient | amount (u64 LE) | nonce | depositor
pub const RECEIPT_LEN: usize = 8 + 32 + 20 + 8 + 32 + 32;
const DEPOSIT_ID_DOMAIN: &[u8] = b"postfiat.pfusdc.solana.deposit_id.v1";
/// Solana's largest account. A longer claim is malformed, not merely expensive.
pub const MAX_ACCOUNT_DATA: usize = 10 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    Truncated,
    TrailingBytes,
    AccountDataTooLarge,
    BankHashMismatch,
    ParentBankHashMismatch,
    DuplicateAccount,
    DeltaMismatch,
    InvalidExecutableFlag,
    /// A new account owned by the vault program that is not a well-formed receipt for this
    /// route. The vault program never creates one, so the witness is refused outright.
    MalformedVaultAccount,
    NoReceipt,
    InvalidRecorderAccount,
}

/// The account lattice sum: 1,024 lanes of 16 bits, added and subtracted lane by lane.
#[derive(Clone, PartialEq, Eq)]
pub struct LtHash(pub [u16; LT_ELEMENTS]);

impl LtHash {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut lanes = [0u16; LT_ELEMENTS];
        for (lane, pair) in lanes.iter_mut().zip(bytes.chunks_exact(2)) {
            *lane = u16::from_le_bytes([pair[0], pair[1]]);
        }
        Self(lanes)
    }
    pub fn to_bytes(&self) -> [u8; LT_BYTES] {
        let mut bytes = [0u8; LT_BYTES];
        for (pair, lane) in bytes.chunks_exact_mut(2).zip(self.0.iter()) {
            pair.copy_from_slice(&lane.to_le_bytes());
        }
        bytes
    }
    fn add(&mut self, other: &[u8; LT_BYTES]) {
        for (lane, pair) in self.0.iter_mut().zip(other.chunks_exact(2)) {
            *lane = lane.wrapping_add(u16::from_le_bytes([pair[0], pair[1]]));
        }
    }
    fn sub(&mut self, other: &[u8; LT_BYTES]) {
        for (lane, pair) in self.0.iter_mut().zip(other.chunks_exact(2)) {
            *lane = lane.wrapping_sub(u16::from_le_bytes([pair[0], pair[1]]));
        }
    }
}

/// One account state as the bank hashes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState<'a> {
    pub lamports: u64,
    pub executable: bool,
    pub owner: [u8; 32],
    pub data: &'a [u8],
}

/// The lattice hash of one account. An account with zero lamports does not exist for the
/// bank and contributes nothing, whatever else the witness says about it.
pub fn account_lt_hash(address: &[u8; 32], state: &AccountState) -> Option<[u8; LT_BYTES]> {
    if state.lamports == 0 {
        return None;
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(&state.lamports.to_le_bytes());
    hasher.update(state.data);
    hasher.update(&[u8::from(state.executable)]);
    hasher.update(&state.owner);
    hasher.update(address);
    let mut out = [0u8; LT_BYTES];
    hasher.finalize_xof().fill(&mut out);
    Some(out)
}

/// The three values hashed with the lattice sum to make a bank hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankHashPreimage {
    pub parent_bank_hash: [u8; 32],
    pub signature_count: u64,
    pub last_blockhash: [u8; 32],
}

pub fn bank_hash(preimage: &BankHashPreimage, accounts: &LtHash) -> [u8; 32] {
    let inner: [u8; 32] = Sha256::new()
        .chain_update(preimage.parent_bank_hash)
        .chain_update(preimage.signature_count.to_le_bytes())
        .chain_update(preimage.last_blockhash)
        .finalize()
        .into();
    Sha256::new().chain_update(inner).chain_update(accounts.to_bytes()).finalize().into()
}

/// A deposit receipt created by the vault program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub address: [u8; 32],
    pub pftl_recipient: [u8; 20],
    pub amount: u64,
    pub nonce: [u8; 32],
    pub depositor: [u8; 32],
    pub deposit_id: [u8; 32],
}

/// What the state layer establishes. `bank_hash` is still an input here: steps 1, 2 and 6
/// are what will tie it to a finalized block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatePublicValues {
    pub bank_hash: [u8; 32],
    pub parent_bank_hash: [u8; 32],
    pub vault_program: [u8; 32],
    pub route_binding: [u8; 32],
    pub changed_accounts: u32,
    pub receipts: Vec<Receipt>,
}

impl StatePublicValues {
    /// Fixed layout: no optional fields and no padding, so one value has one encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(132 + 4 + self.receipts.len() * 156);
        out.extend_from_slice(&self.bank_hash);
        out.extend_from_slice(&self.parent_bank_hash);
        out.extend_from_slice(&self.vault_program);
        out.extend_from_slice(&self.route_binding);
        out.extend_from_slice(&self.changed_accounts.to_le_bytes());
        out.extend_from_slice(&(self.receipts.len() as u32).to_le_bytes());
        for receipt in &self.receipts {
            out.extend_from_slice(&receipt.deposit_id);
            out.extend_from_slice(&receipt.address);
            out.extend_from_slice(&receipt.pftl_recipient);
            out.extend_from_slice(&receipt.amount.to_le_bytes());
            out.extend_from_slice(&receipt.nonce);
            out.extend_from_slice(&receipt.depositor);
        }
        out
    }
}

pub fn deposit_id(vault_program: &[u8; 32], address: &[u8; 32], data: &[u8]) -> [u8; 32] {
    Sha256::new().chain_update(DEPOSIT_ID_DOMAIN).chain_update(vault_program).chain_update(address).chain_update(data).finalize().into()
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(len).ok_or(Error::Truncated)?;
        let slice = self.bytes.get(self.at..end).ok_or(Error::Truncated)?;
        self.at = end;
        Ok(slice)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }
    fn u8(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    fn u32(&mut self) -> Result<u32, Error> { Ok(u32::from_le_bytes(self.array()?)) }
    fn u64(&mut self) -> Result<u64, Error> { Ok(u64::from_le_bytes(self.array()?)) }
    fn preimage(&mut self) -> Result<BankHashPreimage, Error> {
        Ok(BankHashPreimage { parent_bank_hash: self.array()?, signature_count: self.u64()?, last_blockhash: self.array()? })
    }
    fn lt_hash(&mut self) -> Result<LtHash, Error> { Ok(LtHash::from_bytes(self.take(LT_BYTES)?)) }
    fn account(&mut self) -> Result<AccountState<'a>, Error> {
        let lamports = self.u64()?;
        // Any non-zero byte is "executable" to a careless reader; accept only the two values Solana writes.
        let executable = match self.u8()? { 0 => false, 1 => true, _ => return Err(Error::InvalidExecutableFlag) };
        let owner = self.array()?;
        let len = self.u32()? as usize;
        if len > MAX_ACCOUNT_DATA { return Err(Error::AccountDataTooLarge); }
        Ok(AccountState { lamports, executable, owner, data: self.take(len)? })
    }
}

/// Witness layout (all integers little-endian):
/// `bank_hash | vault_program | route_binding | preimage | lt_after | parent preimage |
///  lt_before | count u32 | count x (address | old account | new account)`
/// where an account is `lamports u64 | executable u8 | owner | data_len u32 | data`.
pub fn verify_state_witness(witness: &[u8]) -> Result<StatePublicValues, Error> {
    verify_state(witness, true)
}

/// `require_receipt = false` is the capture-inspection mode: every check is the same, but a
/// block with no deposit receipt is reported instead of refused. Deposit proofs never use it.
fn verify_state(witness: &[u8], require_receipt: bool) -> Result<StatePublicValues, Error> {
    let mut reader = Reader { bytes: witness, at: 0 };
    let claimed_bank_hash: [u8; 32] = reader.array()?;
    let vault_program: [u8; 32] = reader.array()?;
    let route_binding: [u8; 32] = reader.array()?;

    let mut receipts = Vec::new();
    let (parent_bank_hash, count) = verify_delta(&mut reader, claimed_bank_hash, |address, old, new| {
        // Step 5. "Did not exist before" is old lamports zero: such an account contributes
        // nothing to the sum, so the rest of its claimed old state is irrelevant.
        if old.lamports == 0 && new.lamports > 0 && new.owner == vault_program {
            let data = new.data;
            if new.executable || data.len() != RECEIPT_LEN || &data[..8] != RECEIPT_MAGIC || data[8..40] != route_binding {
                return Err(Error::MalformedVaultAccount);
            }
            let amount = u64::from_le_bytes(data[60..68].try_into().expect("length checked"));
            if amount == 0 { return Err(Error::MalformedVaultAccount); }
            receipts.push(Receipt {
                address,
                pftl_recipient: data[40..60].try_into().expect("length checked"),
                amount,
                nonce: data[68..100].try_into().expect("length checked"),
                depositor: data[100..132].try_into().expect("length checked"),
                deposit_id: deposit_id(&vault_program, &address, data),
            });
        }
        Ok(())
    })?;
    if require_receipt && receipts.is_empty() { return Err(Error::NoReceipt); }

    Ok(StatePublicValues {
        bank_hash: claimed_bank_hash,
        parent_bank_hash,
        vault_program,
        route_binding,
        changed_accounts: count,
        receipts,
    })
}

/// Opens both bank hashes and visits each distinct changed account. Callers may use
/// the visited data only after the complete delta verifies.
fn verify_delta<'a>(
    reader: &mut Reader<'a>,
    claimed_bank_hash: [u8; 32],
    mut visit: impl FnMut([u8; 32], AccountState<'a>, AccountState<'a>) -> Result<(), Error>,
) -> Result<([u8; 32], u32), Error> {
    // Step 3.
    let preimage = reader.preimage()?;
    let lt_after = reader.lt_hash()?;
    if bank_hash(&preimage, &lt_after) != claimed_bank_hash { return Err(Error::BankHashMismatch); }
    let parent_preimage = reader.preimage()?;
    let lt_before = reader.lt_hash()?;
    if bank_hash(&parent_preimage, &lt_before) != preimage.parent_bank_hash { return Err(Error::ParentBankHashMismatch); }

    // Step 4.
    let count = reader.u32()?;
    let mut running = lt_before;
    let mut addresses: Vec<[u8; 32]> = Vec::with_capacity(count.min(1 << 16) as usize);
    for _ in 0..count {
        let address: [u8; 32] = reader.array()?;
        let old = reader.account()?;
        let new = reader.account()?;
        if let Some(hash) = account_lt_hash(&address, &old) { running.sub(&hash); }
        if let Some(hash) = account_lt_hash(&address, &new) { running.add(&hash); }
        addresses.push(address);
        visit(address, old, new)?;
    }
    if reader.at != reader.bytes.len() { return Err(Error::TrailingBytes); }
    addresses.sort_unstable();
    if addresses.windows(2).any(|pair| pair[0] == pair[1]) { return Err(Error::DuplicateAccount); }
    if running != lt_after { return Err(Error::DeltaMismatch); }
    Ok((preimage.parent_bank_hash, count))
}

/// Steps 1 to 5 joined: what a finalized block ID commits to.
///
/// The block ID is still not shown to be finalized: step 6, the certificate, is not built.
/// Until it is, these values say "if Solana finalized this block ID, these deposits happened".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPublicValues {
    pub slot: u64,
    pub block_id: [u8; 32],
    pub shred_version: u16,
    pub state: StatePublicValues,
}

impl BlockPublicValues {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.slot.to_le_bytes());
        out.extend_from_slice(&self.block_id);
        out.extend_from_slice(&self.shred_version.to_le_bytes());
        out.extend_from_slice(&self.state.encode());
        out
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BlockError {
    Truncated,
    Shreds(shreds::ShredError),
    State(Error),
    /// The state witness opens a bank hash other than the one in the block's footer.
    BankHashNotInFooter,
}

/// Witness layout: `slot u64 | parent_slot u64 | parent_block_id | shred_count u32 |
/// shred_count x 1203-byte data shred payloads | state witness` (see `verify_state_witness`).
pub fn verify_block_witness(witness: &[u8]) -> Result<BlockPublicValues, BlockError> {
    verify_block(witness, true)
}

fn verify_block(witness: &[u8], require_receipt: bool) -> Result<BlockPublicValues, BlockError> {
    let (block, bytes) = open_block_witness(witness)?;
    let state = verify_state(bytes, require_receipt).map_err(BlockError::State)?;
    Ok(BlockPublicValues { slot: block.slot, block_id: block.block_id, shred_version: block.shred_version, state })
}

struct BlockHeader {
    slot: u64,
    block_id: [u8; 32],
    shred_version: u16,
}

/// Shared shred and footer checks. Each caller opens the returned state bytes with verify_delta.
fn open_block_witness(witness: &[u8]) -> Result<(BlockHeader, &[u8]), BlockError> {
    let mut reader = Reader { bytes: witness, at: 0 };
    let slot = reader.u64().map_err(|_| BlockError::Truncated)?;
    let parent_slot = reader.u64().map_err(|_| BlockError::Truncated)?;
    let parent_block_id: [u8; 32] = reader.array().map_err(|_| BlockError::Truncated)?;
    let count = reader.u32().map_err(|_| BlockError::Truncated)? as usize;
    let bytes = count.checked_mul(shreds::SHRED_PAYLOAD).ok_or(BlockError::Truncated)?;
    let payloads = reader.take(bytes).map_err(|_| BlockError::Truncated)?;
    let views: Vec<&[u8]> = payloads.chunks_exact(shreds::SHRED_PAYLOAD).collect();
    let block = shreds::block_from_shreds(slot, parent_slot, &parent_block_id, &views).map_err(BlockError::Shreds)?;
    let footer = shreds::footer_bank_hash(&block).map_err(BlockError::Shreds)?;
    let bytes = &witness[reader.at..];
    if bytes.get(..32).ok_or(BlockError::Truncated)? != footer { return Err(BlockError::BankHashNotInFooter); }
    Ok((BlockHeader { slot, block_id: block.block_id, shred_version: block.shred_version }, bytes))
}

/// The complete deposit statement, steps 1 to 6: these receipts were created in a block that
/// the validator set with this hash finalized.
///
/// The ledger must still check that `validator_set_hash` and `total_stake` are the ones its
/// Solana light client holds for the block's epoch, that the vault program and route binding
/// are the route's, and that each deposit ID is new. Nothing here authenticates the set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositPublicValues {
    pub validator_set_hash: [u8; 32],
    pub total_stake: u128,
    /// True when one 80% certificate established finality; false for the two 60% certificates.
    pub fast_finality: bool,
    pub block: BlockPublicValues,
}

impl DepositPublicValues {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"PFSOLDP1");
        out.extend_from_slice(&self.validator_set_hash);
        out.extend_from_slice(&self.total_stake.to_le_bytes());
        out.push(u8::from(self.fast_finality));
        out.extend_from_slice(&self.block.encode());
        out
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DepositError {
    Truncated,
    UnknownFinalityKind,
    Certificate(certificate::CertificateError),
    Block(BlockError),
}

/// Witness layout:
/// `total_stake u128 | validator_count u32 | validator_count x (pubkey 48 | stake u64) |
///  finality kind u8 (0 fast, 1 slow) | aggregate | [second aggregate when slow] | block witness`
/// where an aggregate is `signature 192 | bitmap_len u16 | bitmap`, the slow form lists the
/// notarization aggregate first, and the block witness is as in `verify_block_witness`.
pub fn verify_deposit_witness(witness: &[u8]) -> Result<DepositPublicValues, DepositError> {
    verify_deposit(witness, true)
}

/// Native capture inspection. Checks the same bank, shreds and finality certificate as deposit
/// verification, including receipt decoding, but accepts an empty receipt list. No guest runs it.
pub fn verify_finalized_block_witness(witness: &[u8]) -> Result<DepositPublicValues, DepositError> {
    verify_deposit(witness, false)
}

fn verify_deposit(witness: &[u8], require_receipt: bool) -> Result<DepositPublicValues, DepositError> {
    let mut reader = Reader { bytes: witness, at: 0 };
    let set = reader.validator_set()?;
    let finality = reader.finality()?;
    let total_stake = set.total_stake;
    // The block first: the certificate is checked against the block ID the shreds give, never one the witness names.
    let block = verify_block(&witness[reader.at..], require_receipt).map_err(DepositError::Block)?;
    certificate::verify_finality(&set, block.slot, &block.block_id, block.shred_version, &finality).map_err(DepositError::Certificate)?;
    Ok(DepositPublicValues { validator_set_hash: set.hash(), total_stake, fast_finality: matches!(finality, certificate::Finality::Fast(_)), block })
}

impl<'a> Reader<'a> {
    fn validator_set(&mut self) -> Result<certificate::ValidatorSet, DepositError> {
        let truncated = |_| DepositError::Truncated;
        let total = u128::from_le_bytes(self.array().map_err(truncated)?);
        let count = self.u32().map_err(truncated)? as usize;
        if count > certificate::MAX_VALIDATORS {
            return Err(DepositError::Certificate(certificate::CertificateError::TooManyValidators));
        }
        let mut validators = Vec::with_capacity(count);
        for _ in 0..count {
            validators.push(certificate::Validator {
                pubkey: self.array().map_err(truncated)?,
                stake: self.u64().map_err(truncated)?,
            });
        }
        certificate::ValidatorSet::new(validators, total).map_err(DepositError::Certificate)
    }

    fn aggregate(&mut self) -> Result<certificate::Aggregate<'a>, Error> {
        let signature = self.take(certificate::SIGNATURE_LEN)?.try_into().expect("fixed length");
        let len = u16::from_le_bytes(self.array()?);
        Ok(certificate::Aggregate { signature, bitmap: self.take(usize::from(len))? })
    }

    fn finality(&mut self) -> Result<certificate::Finality<'a>, DepositError> {
        let truncated = |_| DepositError::Truncated;
        match self.u8().map_err(truncated)? {
            0 => Ok(certificate::Finality::Fast(self.aggregate().map_err(truncated)?)),
            1 => Ok(certificate::Finality::Slow {
                notarize: self.aggregate().map_err(truncated)?,
                finalize: self.aggregate().map_err(truncated)?,
            }),
            _ => Err(DepositError::UnknownFinalityKind),
        }
    }
}

#[cfg(test)]
mod tests;
