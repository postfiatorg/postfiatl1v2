//! Native pfUSDC deposit vault. Instruction and account layouts are in ../README.md.
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_program_entrypoint::entrypoint;
use solana_program_error::{ProgramError, ProgramResult};
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;
use solana_system_interface::instruction as system;
use solana_sysvar::{clock::Clock, rent::Rent, Sysvar};
use spl_token_interface::{
    instruction as token,
    state::{Account as TokenAccount, Mint},
};

entrypoint!(process_instruction);

pub const RECEIPT_MAGIC: &[u8; 8] = b"PFSOLRC1";
pub const RECEIPT_LEN: usize = 132;
const CONFIG_LEN: usize = 96;
const CONFIG: &[u8] = b"config";
const CUSTODY: &[u8] = b"custody";
const RECEIPT: &[u8] = b"receipt";
const DECIMALS: u8 = 6;

pub fn process_instruction(id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    match data.split_first() {
        Some((0, args)) if args.len() == 48 => initialize(id, accounts, args),
        Some((1, args)) if args.len() == 60 => deposit(id, accounts, args),
        Some((2, args)) => pfusdc_egress_verifier::vault::release(
            id, &Pubkey::new_from_array([107, 83, 226, 189, 147, 15, 167, 240, 182, 232, 164, 13, 92, 160, 101, 58, 243, 204, 24, 111, 175, 113, 155, 201, 19, 180, 78, 191, 28, 71, 79, 52]), accounts, args,
        ),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn u64_le(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("fixed-width field"))
}

fn pda(id: &Pubkey, key: &Pubkey, seeds: &[&[u8]]) -> Result<u8, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(seeds, id);
    if key != &expected {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(bump)
}

fn programs(system: &AccountInfo, token: &AccountInfo) -> ProgramResult {
    if system.key != &system_program::ID || token.key != &spl_token_interface::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

struct Creation<'a, 'info> {
    payer: &'a AccountInfo<'info>,
    system: &'a AccountInfo<'info>,
    rent: Rent,
}

impl<'info> Creation<'_, 'info> {
    fn create(
        &self,
        account: &AccountInfo<'info>,
        owner: &Pubkey,
        size: usize,
        seeds: &[&[u8]],
    ) -> ProgramResult {
        let ix = system::create_account_allow_prefund(
            account.key,
            Some((
                self.payer.key,
                self.rent
                    .minimum_balance(size)
                    .saturating_sub(account.lamports()),
            )),
            size as u64,
            owner,
        );
        invoke_signed(
            &ix,
            &[account.clone(), self.payer.clone(), self.system.clone()],
            &[seeds],
        )
    }
}

fn initialize(id: &Pubkey, accounts: &[AccountInfo], args: &[u8]) -> ProgramResult {
    let [payer, deployment, config, custody, mint, token_program, system] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if deployment.key != id || !deployment.is_signer || !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    programs(system, token_program)?;
    let config_bump = pda(id, config.key, &[CONFIG])?;
    let custody_bump = pda(id, custody.key, &[CUSTODY])?;
    if config.owner != &system_program::ID || custody.owner != &system_program::ID {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    let per_deposit = u64_le(&args[32..40]);
    let total = u64_le(&args[40..48]);
    if per_deposit == 0 || total < per_deposit {
        return Err(ProgramError::InvalidArgument);
    }
    if mint.owner != token_program.key
        || Mint::unpack(&mint.try_borrow_data()?)?.decimals != DECIMALS
    {
        return Err(ProgramError::InvalidAccountData);
    }

    let creation = Creation {
        payer,
        system,
        rent: Rent::get()?,
    };
    creation.create(config, id, CONFIG_LEN, &[CONFIG, &[config_bump]])?;
    creation.create(
        custody,
        token_program.key,
        TokenAccount::LEN,
        &[CUSTODY, &[custody_bump]],
    )?;
    invoke(
        &token::initialize_account3(token_program.key, custody.key, mint.key, config.key)?,
        &[custody.clone(), mint.clone(), token_program.clone()],
    )?;
    let mut state = config.try_borrow_mut_data()?;
    state[..32].copy_from_slice(mint.key.as_ref());
    state[32..80].copy_from_slice(args);
    state[80..88].copy_from_slice(&0u64.to_le_bytes());
    state[88..96].copy_from_slice(&Clock::get()?.slot.to_le_bytes());
    Ok(())
}

fn deposit(id: &Pubkey, accounts: &[AccountInfo], args: &[u8]) -> ProgramResult {
    let [depositor, source, config, custody, mint, receipt, token_program, system] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    programs(system, token_program)?;
    if !depositor.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    pda(id, config.key, &[CONFIG])?;
    pda(id, custody.key, &[CUSTODY])?;
    let nonce = &args[28..60];
    let receipt_bump = pda(id, receipt.key, &[RECEIPT, nonce])?;
    // The ingress proof requires old lamports == 0. A prefunded nonce is unusable.
    if receipt.lamports() != 0 || !receipt.data_is_empty() || receipt.owner != &system_program::ID {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    if config.owner != id
        || config.data_len() != CONFIG_LEN
        || source.key == custody.key
        || source.owner != token_program.key
        || custody.owner != token_program.key
        || mint.owner != token_program.key
    {
        return Err(ProgramError::InvalidAccountData);
    }
    let amount = u64_le(&args[..8]);
    let (route, new_total) = {
        let state = config.try_borrow_data()?;
        if &state[..32] != mint.key.as_ref()
            || TokenAccount::unpack(&source.try_borrow_data()?)?.owner != *depositor.key
        {
            return Err(ProgramError::InvalidAccountData);
        }
        // Ingress treats every newly created vault-owned account as a receipt. Keep
        // configuration creation out of any bank in which a deposit can succeed.
        if Clock::get()?.slot <= u64_le(&state[88..96]) {
            return Err(ProgramError::InvalidArgument);
        }
        let new_total = u64_le(&state[80..88])
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        if amount == 0 || amount > u64_le(&state[64..72]) || new_total > u64_le(&state[72..80]) {
            return Err(ProgramError::InvalidArgument);
        }
        (<[u8; 32]>::try_from(&state[32..64]).unwrap(), new_total)
    };

    invoke(
        &token::transfer_checked(
            token_program.key,
            source.key,
            mint.key,
            custody.key,
            depositor.key,
            &[],
            amount,
            DECIMALS,
        )?,
        &[
            source.clone(),
            mint.clone(),
            custody.clone(),
            depositor.clone(),
            token_program.clone(),
        ],
    )?;
    Creation {
        payer: depositor,
        system,
        rent: Rent::get()?,
    }
    .create(receipt, id, RECEIPT_LEN, &[RECEIPT, nonce, &[receipt_bump]])?;
    // programs/pfusdc-sol-ingress/src/lib.rs::verify_state_witness
    let mut out = receipt.try_borrow_mut_data()?;
    out[..8].copy_from_slice(RECEIPT_MAGIC);
    out[8..40].copy_from_slice(&route);
    out[40..60].copy_from_slice(&args[8..28]);
    out[60..68].copy_from_slice(&args[..8]);
    out[68..100].copy_from_slice(nonce);
    out[100..].copy_from_slice(depositor.key.as_ref());
    config.try_borrow_mut_data()?[80..88].copy_from_slice(&new_total.to_le_bytes());
    Ok(())
}
