//! Vote-key and epoch-stake batches. See solana/README.md for the wire format.
use solana_account_info::AccountInfo;
use solana_clock::Clock;
use solana_cpi::invoke;
use solana_epoch_stake::{get_epoch_stake_for_vote_account, get_epoch_total_stake};
use solana_program_entrypoint::entrypoint;
use solana_program_error::{ProgramError, ProgramResult};
use solana_pubkey::Pubkey;
use solana_sdk_ids::{system_program, vote};
use solana_system_interface::instruction::{create_account, transfer};
use solana_sysvar::{rent::Rent, Sysvar};

entrypoint!(process_instruction);

pub const KEY_MAGIC: &[u8; 8] = b"PFSOLKY1";
pub const SCHEDULED_STAKE_MAGIC: &[u8; 8] = b"PFSOLNX1";
pub const STAKE_MAGIC: &[u8; 8] = b"PFSOLST1";
pub const KEY_HEADER_LEN: usize = 20;
pub const STAKE_HEADER_LEN: usize = 28;
pub const KEY_ROW_LEN: usize = 48;
pub const STAKE_ROW_LEN: usize = 120;
pub const MAX_BATCH: usize = 32;

// Agave 141da0c3 vote/src/vote_state_view/frame_v4.rs, bls_pubkey_compressed_offset.
// VoteStateV4::size_of() = 3762. Only the vote program can write this data.
// Its programs/vote/src/vote_state/mod.rs verifies PoP on BLS registration.
fn read_vote(account: &AccountInfo) -> Result<([u8; 32], [u8; 48]), ProgramError> {
    if account.owner != &vote::ID || account.executable {
        return Err(ProgramError::IncorrectProgramId);
    }
    let data = account.try_borrow_data()?;
    const OPTION_OFFSET: usize = 4 + 4 * 32 + 2 * 2 + 8;
    if data.len() != 3762 || data[..4] != 3u32.to_le_bytes() || data[OPTION_OFFSET] != 1 {
        return Err(ProgramError::InvalidAccountData);
    }
    let node = data[4..36].try_into().unwrap();
    let key = data[OPTION_OFFSET + 1..OPTION_OFFSET + 49]
        .try_into()
        .unwrap();
    Ok((node, key))
}

pub fn process_instruction(id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Alpenglow votes in E use epoch_stakes[E]. The SBF stake syscall in E-1
    // reads epoch_stakes[E]. Publish those frozen rows when E actually starts.
    if data == [2] {
        let [output] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if output.owner != id {
            return Err(ProgramError::IncorrectProgramId);
        }
        let mut bytes = output.try_borrow_mut_data()?;
        if bytes.len() < STAKE_HEADER_LEN || &bytes[..8] != SCHEDULED_STAKE_MAGIC
            || bytes[8..16] != Clock::get()?.epoch.to_le_bytes()
        {
            return Err(ProgramError::InvalidAccountData);
        }
        bytes[..8].copy_from_slice(STAKE_MAGIC);
        return Ok(());
    }
    let (magic, header, stride) = match data {
        [0] => (KEY_MAGIC, KEY_HEADER_LEN, KEY_ROW_LEN),
        [1] => (SCHEDULED_STAKE_MAGIC, STAKE_HEADER_LEN, STAKE_ROW_LEN),
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    let [payer, output, system, votes @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if votes.is_empty() || votes.len() > MAX_BATCH {
        return Err(ProgramError::InvalidArgument);
    }
    if !payer.is_signer || !output.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if system.key != &system_program::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let stakes = data == [1];
    let epoch = Clock::get()?.epoch.checked_add(u64::from(stakes))
        .ok_or(ProgramError::ArithmeticOverflow)?;
    // Agave 141da0c3 syscalls/src/lib.rs, SyscallGetEpochStake; runtime/src/bank.rs,
    // get_current_epoch_total_stake and get_epoch_stake_for_vote_account.
    let total = if stakes { get_epoch_total_stake() } else { 0 };
    let rent = Rent::get()?;
    let count_at = header - 4;
    let prior = if output.owner == id {
        let old = output.try_borrow_data()?;
        if old.len() < header
            || &old[..8] != magic
            || old[8..16] != epoch.to_le_bytes()
            || (stakes && old[16..24] != total.to_le_bytes())
        {
            return Err(ProgramError::InvalidAccountData);
        }
        let count = u32::from_le_bytes(old[count_at..header].try_into().unwrap()) as usize;
        if old.len() != header + count * stride || count == 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        count
    } else {
        0
    };
    let count = prior + votes.len();
    let size = header + count * stride;
    if prior == 0 {
        invoke(
            &create_account(
                payer.key,
                output.key,
                rent.minimum_balance(size),
                size as u64,
                id,
            ),
            &[payer.clone(), output.clone(), system.clone()],
        )?;
    } else {
        let top_up = rent.minimum_balance(size).saturating_sub(output.lamports());
        if top_up > 0 {
            invoke(
                &transfer(payer.key, output.key, top_up),
                &[payer.clone(), output.clone(), system.clone()],
            )?;
        }
        output.resize(size)?;
    }
    let mut out = output.try_borrow_mut_data()?;
    out[..8].copy_from_slice(magic);
    out[8..16].copy_from_slice(&epoch.to_le_bytes());
    if stakes {
        out[16..24].copy_from_slice(&total.to_le_bytes());
    }
    out[count_at..header].copy_from_slice(&(count as u32).to_le_bytes());
    for (i, account) in votes.iter().enumerate() {
        let (node, key) = read_vote(account)?;
        let start = header + (prior + i) * stride;
        if stakes {
            if out[header..start]
                .chunks_exact(stride)
                .any(|row| &row[..32] == account.key.as_ref())
            {
                return Err(ProgramError::InvalidArgument);
            }
            let row = &mut out[start..start + stride];
            row[..32].copy_from_slice(account.key.as_ref());
            row[32..64].copy_from_slice(&node);
            row[64..72]
                .copy_from_slice(&get_epoch_stake_for_vote_account(account.key).to_le_bytes());
            row[72..].copy_from_slice(&key);
        } else {
            out[start..start + stride].copy_from_slice(&key);
        }
    }
    Ok(())
}
