//! Link only this callback into the deposit vault's tag 2. Its verifier ID is a
//! build-time pin in the calling vault, never an instruction/account argument.
use crate::{pda, Error, ATA_PROGRAM, USDC};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_program_error::{ProgramError, ProgramResult};
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use spl_token_interface::{instruction as token, state::Account as TokenAccount};

pub const AUTHORITY: &[u8] = b"egress-release";

pub fn release(
    vault: &Pubkey,
    verifier: &Pubkey,
    accounts: &[AccountInfo],
    args: &[u8],
) -> ProgramResult {
    let [authority, config, custody, ata, owner, mint, token_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if args.len() != 40 {
        return Err(Error::Encoding.into());
    }
    let amount = u64::from_le_bytes(args[..8].try_into().unwrap());
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    pda(verifier, authority, &[AUTHORITY, vault.as_ref()])?;
    let config_bump = pda(vault, config, &[b"config"])?;
    pda(vault, custody, &[b"custody"])?;
    if amount == 0
        || config.owner != vault
        || config.data_len() != 96
        || &config.try_borrow_data()?[..32] != USDC.as_ref()
        || config.try_borrow_data()?[32..64] != args[8..]
        || mint.key != &USDC
        || token_program.key != &spl_token_interface::ID
        || mint.owner != token_program.key
        || custody.owner != token_program.key
        || ata.owner != token_program.key
        || ata.key == custody.key
    {
        return Err(Error::Binding.into());
    }
    let expected = Pubkey::find_program_address(
        &[
            owner.key.as_ref(),
            token_program.key.as_ref(),
            mint.key.as_ref(),
        ],
        &ATA_PROGRAM,
    )
    .0;
    let dest = TokenAccount::unpack(&ata.try_borrow_data()?)?;
    if *ata.key != expected || dest.owner != *owner.key || dest.mint != USDC {
        return Err(Error::WrongDestination.into());
    }
    invoke_signed(
        &token::transfer_checked(
            token_program.key,
            custody.key,
            mint.key,
            ata.key,
            config.key,
            &[],
            amount,
            6,
        )?,
        &[
            custody.clone(),
            mint.clone(),
            ata.clone(),
            config.clone(),
            token_program.clone(),
        ],
        &[&[b"config", &[config_bump]]],
    )
}
