//! Vote-key and epoch-stake batches. See solana/README.md for the wire format.
use solana_account_info::AccountInfo;
use solana_clock::Clock;
use solana_cpi::{invoke, invoke_signed};
use solana_epoch_stake::{get_epoch_stake_for_vote_account, get_epoch_total_stake};
use solana_program_entrypoint::entrypoint;
use solana_program_error::{ProgramError, ProgramResult};
use solana_pubkey::{pubkey, Pubkey};
use solana_sdk_ids::{system_program, vote};
use solana_system_interface::instruction::{
    create_account, create_account_allow_prefund, transfer,
};
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

const FUNDER: &[u8] = b"funder";
const LEGACY_KEYS: Pubkey = pubkey!("9pzu1VcaZSWGbn9aBx4iq39V3vnjsXFC9w4QBQoPdgeC");
const LEGACY_STAKES: Pubkey = pubkey!("G3YEW7FjXUoEkc1MVpXuotkDnXH69BQLcZkiAuM1sZZB");
const LEGACY_PAYER: Pubkey = pubkey!("4JJEuaYtJvgtMVixTbXfppHJPh6tcU5g7g7UtkvxK3sB");

fn funder_address(id: &Pubkey, table: &Pubkey, account: &AccountInfo) -> Result<u8, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(&[FUNDER, table.as_ref()], id);
    if account.key != &expected {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(bump)
}

fn refund(account: &AccountInfo, recipient: &AccountInfo) -> ProgramResult {
    let balance = recipient
        .lamports()
        .checked_add(account.lamports())
        .ok_or(ProgramError::ArithmeticOverflow)?;
    account.resize(0)?;
    account.assign(&system_program::ID);
    **recipient.try_borrow_mut_lamports()? = balance;
    **account.try_borrow_mut_lamports()? = 0;
    Ok(())
}

fn close(id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let [table, recipient, metadata @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !table.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if table.owner != id || table.executable || table.key == recipient.key {
        return Err(ProgramError::InvalidAccountData);
    }
    let data = table.try_borrow_data()?;
    let (header, width, keys) = match data.get(..8) {
        Some(tag) if tag == KEY_MAGIC => (KEY_HEADER_LEN, KEY_ROW_LEN, true),
        Some(tag) if tag == SCHEDULED_STAKE_MAGIC || tag == STAKE_MAGIC => {
            (STAKE_HEADER_LEN, STAKE_ROW_LEN, false)
        }
        _ => return Err(ProgramError::InvalidAccountData),
    };
    if data.len() < header {
        return Err(ProgramError::InvalidAccountData);
    }
    let count = u32::from_le_bytes(data[header - 4..header].try_into().unwrap()) as usize;
    if count == 0 || data.len() != header + count * width {
        return Err(ProgramError::InvalidAccountData);
    }
    let epoch = u64::from_le_bytes(data[8..16].try_into().unwrap());
    // Keys are tagged E; both scheduled and published stakes already carry E+1.
    let successor = epoch
        .checked_add(u64::from(keys))
        .ok_or(ProgramError::ArithmeticOverflow)?;
    if Clock::get()?.epoch <= successor {
        return Err(ProgramError::InvalidArgument);
    }
    drop(data);
    match metadata {
        [] if successor == 1091
            && recipient.key == &LEGACY_PAYER
            && table.key == (if keys { &LEGACY_KEYS } else { &LEGACY_STAKES }) => {}
        [funder] => {
            funder_address(id, table.key, funder)?;
            if funder.owner != id
                || funder.key == recipient.key
                || *funder.try_borrow_data()? != recipient.key.as_ref()
            {
                return Err(ProgramError::InvalidAccountData);
            }
            refund(funder, recipient)?;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }
    refund(table, recipient)
}

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
    if data == [3] {
        return close(id, accounts);
    }
    if data == [2] {
        let [output] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if output.owner != id {
            return Err(ProgramError::IncorrectProgramId);
        }
        let mut bytes = output.try_borrow_mut_data()?;
        if bytes.len() < STAKE_HEADER_LEN
            || &bytes[..8] != SCHEDULED_STAKE_MAGIC
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
    let [payer, output, system, remaining @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    // Only creation needs the extra writable PDA. Existing append and publication
    // instructions retain their wire format, including for pre-upgrade accounts.
    let creating = output.owner != id;
    let (funder, votes) = if creating {
        let (funder, votes) = remaining
            .split_first()
            .ok_or(ProgramError::NotEnoughAccountKeys)?;
        let bump = funder_address(id, output.key, funder)?;
        (Some((funder, bump)), votes)
    } else {
        (None, remaining)
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
    let epoch = Clock::get()?
        .epoch
        .checked_add(u64::from(stakes))
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
    if let Some((funder, bump)) = funder {
        if output.key == &LEGACY_KEYS || output.key == &LEGACY_STAKES {
            return Err(ProgramError::InvalidArgument);
        }
        invoke_signed(
            &create_account_allow_prefund(
                funder.key,
                Some((
                    payer.key,
                    rent.minimum_balance(32).saturating_sub(funder.lamports()),
                )),
                32,
                id,
            ),
            &[funder.clone(), payer.clone(), system.clone()],
            &[&[FUNDER, output.key.as_ref(), &[bump]]],
        )?;
        funder
            .try_borrow_mut_data()?
            .copy_from_slice(payer.key.as_ref());
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
