#![allow(unexpected_cfgs)]

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint,
    entrypoint::ProgramResult,
    hash::hash,
    log::sol_log_data,
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{self, SysvarSerialize},
};

pub const INSTRUCTION_MAGIC: &[u8; 8] = b"PFSOL001";
pub const SNAPSHOT_MAGIC: &[u8; 8] = b"PFSNAP01";
pub const SNAPSHOT_VERSION: u16 = 1;
pub const MAX_STAKE_ACCOUNTS: usize = 32;
pub const STAKE_STATE_V2_DELEGATED: u32 = 2;
pub const DEACTIVATION_EPOCH_DISABLED: u64 = u64::MAX;

const MIN_STAKE_DATA_BYTES: usize = 180;
const STAKE_AUTHORITY_OFFSET: usize = 12;
const WITHDRAW_AUTHORITY_OFFSET: usize = 44;
const VOTE_ACCOUNT_OFFSET: usize = 124;
const DELEGATED_LAMPORTS_OFFSET: usize = 156;
const ACTIVATION_EPOCH_OFFSET: usize = 164;
const DEACTIVATION_EPOCH_OFFSET: usize = 172;
const STAKE_PROGRAM_ID: Pubkey =
    solana_program::pubkey!("Stake11111111111111111111111111111111111111");

entrypoint!(process_instruction);

/// Reads an exact caller-supplied set of standard stake accounts and emits a
/// canonical snapshot. The transaction message commits the ordered account
/// list; the public reserve verifier checks this output and immutable reader
/// identity under a governed finalized-source checkpoint.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (salt, expected_count) = parse_instruction(instruction_data)?;
    if expected_count == 0 || expected_count > MAX_STAKE_ACCOUNTS {
        return Err(ProgramError::InvalidInstructionData);
    }
    let mut accounts = accounts.iter();
    let clock_account = next_account_info(&mut accounts)?;
    if clock_account.key != &sysvar::clock::ID
        || clock_account.is_writable
        || clock_account.executable
    {
        return Err(ProgramError::InvalidArgument);
    }
    let clock = Clock::from_account_info(clock_account)?;
    let stake_accounts = accounts.collect::<Vec<_>>();
    if stake_accounts.len() != expected_count {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let mut payload = Vec::with_capacity(64 + expected_count * 196);
    payload.extend_from_slice(SNAPSHOT_MAGIC);
    payload.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
    payload.extend_from_slice(&clock.slot.to_le_bytes());
    payload.extend_from_slice(&clock.epoch.to_le_bytes());
    payload.extend_from_slice(&salt);
    payload.extend_from_slice(&(expected_count as u16).to_le_bytes());

    let mut previous = None;
    for account in stake_accounts {
        if account.owner != &STAKE_PROGRAM_ID || account.executable || account.is_writable {
            return Err(ProgramError::InvalidAccountData);
        }
        if previous >= Some(account.key.to_bytes()) {
            return Err(ProgramError::InvalidArgument);
        }
        previous = Some(account.key.to_bytes());
        let data = account.try_borrow_data()?;
        let parsed = parse_stake_account(&data)?;
        let lamports = account.lamports();
        if parsed.delegated_lamports > lamports {
            return Err(ProgramError::InvalidAccountData);
        }
        payload.extend_from_slice(account.key.as_ref());
        payload.extend_from_slice(&lamports.to_le_bytes());
        payload.extend_from_slice(account.owner.as_ref());
        payload.extend_from_slice(hash(&data).as_ref());
        payload.extend_from_slice(&parsed.stake_authority);
        payload.extend_from_slice(&parsed.withdraw_authority);
        payload.extend_from_slice(&parsed.vote_account);
        payload.extend_from_slice(&parsed.delegated_lamports.to_le_bytes());
        payload.extend_from_slice(&parsed.activation_epoch.to_le_bytes());
        payload.extend_from_slice(&parsed.deactivation_epoch.to_le_bytes());
    }
    sol_log_data(&[&payload]);
    set_return_data(hash(&payload).as_ref());
    Ok(())
}

fn parse_instruction(data: &[u8]) -> Result<([u8; 32], usize), ProgramError> {
    if data.len() != 42 || &data[..8] != INSTRUCTION_MAGIC {
        return Err(ProgramError::InvalidInstructionData);
    }
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&data[8..40]);
    if salt == [0; 32] {
        return Err(ProgramError::InvalidInstructionData);
    }
    let count = u16::from_le_bytes(
        data[40..42]
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    );
    Ok((salt, usize::from(count)))
}

struct ParsedStakeAccount {
    stake_authority: [u8; 32],
    withdraw_authority: [u8; 32],
    vote_account: [u8; 32],
    delegated_lamports: u64,
    activation_epoch: u64,
    deactivation_epoch: u64,
}

fn parse_stake_account(data: &[u8]) -> Result<ParsedStakeAccount, ProgramError> {
    if data.len() < MIN_STAKE_DATA_BYTES || read_u32(data, 0)? != STAKE_STATE_V2_DELEGATED {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(ParsedStakeAccount {
        stake_authority: read_32(data, STAKE_AUTHORITY_OFFSET)?,
        withdraw_authority: read_32(data, WITHDRAW_AUTHORITY_OFFSET)?,
        vote_account: read_32(data, VOTE_ACCOUNT_OFFSET)?,
        delegated_lamports: read_u64(data, DELEGATED_LAMPORTS_OFFSET)?,
        activation_epoch: read_u64(data, ACTIVATION_EPOCH_OFFSET)?,
        deactivation_epoch: read_u64(data, DEACTIVATION_EPOCH_OFFSET)?,
    })
}

fn read_32(data: &[u8], offset: usize) -> Result<[u8; 32], ProgramError> {
    data.get(offset..offset + 32)
        .ok_or(ProgramError::InvalidAccountData)?
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, ProgramError> {
    data.get(offset..offset + 4)
        .ok_or(ProgramError::InvalidAccountData)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| ProgramError::InvalidAccountData)
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64, ProgramError> {
    data.get(offset..offset + 8)
        .ok_or(ProgramError::InvalidAccountData)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| ProgramError::InvalidAccountData)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_is_exact_and_rejects_zero_salt() {
        let mut instruction = Vec::from(INSTRUCTION_MAGIC);
        instruction.extend_from_slice(&[7; 32]);
        instruction.extend_from_slice(&2u16.to_le_bytes());
        assert_eq!(parse_instruction(&instruction).unwrap(), ([7; 32], 2));
        instruction[8..40].fill(0);
        assert_eq!(
            parse_instruction(&instruction),
            Err(ProgramError::InvalidInstructionData)
        );
    }

    #[test]
    fn stake_parser_uses_canonical_offsets() {
        let mut data = vec![0u8; MIN_STAKE_DATA_BYTES];
        data[..4].copy_from_slice(&STAKE_STATE_V2_DELEGATED.to_le_bytes());
        data[STAKE_AUTHORITY_OFFSET..STAKE_AUTHORITY_OFFSET + 32].fill(1);
        data[WITHDRAW_AUTHORITY_OFFSET..WITHDRAW_AUTHORITY_OFFSET + 32].fill(2);
        data[VOTE_ACCOUNT_OFFSET..VOTE_ACCOUNT_OFFSET + 32].fill(3);
        data[DELEGATED_LAMPORTS_OFFSET..DELEGATED_LAMPORTS_OFFSET + 8]
            .copy_from_slice(&99u64.to_le_bytes());
        data[ACTIVATION_EPOCH_OFFSET..ACTIVATION_EPOCH_OFFSET + 8]
            .copy_from_slice(&10u64.to_le_bytes());
        data[DEACTIVATION_EPOCH_OFFSET..DEACTIVATION_EPOCH_OFFSET + 8]
            .copy_from_slice(&DEACTIVATION_EPOCH_DISABLED.to_le_bytes());
        let parsed = parse_stake_account(&data).unwrap();
        assert_eq!(parsed.stake_authority, [1; 32]);
        assert_eq!(parsed.withdraw_authority, [2; 32]);
        assert_eq!(parsed.vote_account, [3; 32]);
        assert_eq!(parsed.delegated_lamports, 99);
        assert_eq!(parsed.activation_epoch, 10);
        assert_eq!(parsed.deactivation_epoch, DEACTIVATION_EPOCH_DISABLED);
    }

    #[test]
    fn malformed_stake_state_fails_closed() {
        assert!(matches!(
            parse_stake_account(&[0; MIN_STAKE_DATA_BYTES]),
            Err(ProgramError::InvalidAccountData)
        ));
    }
}
