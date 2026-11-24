//! Vault instruction handlers. Compile with no-entrypoint when linked into the deposit vault.
use borsh::{BorshDeserialize, BorshSerialize};
use sha3::{Digest, Keccak256, Sha3_384};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::{ProgramError, ProgramResult};
use solana_pubkey::{pubkey, Pubkey};
use solana_sdk_ids::system_program;
use solana_sha256_hasher::hash;
use solana_system_interface::instruction as system;
use solana_sysvar::{rent::Rent, Sysvar};

mod groth16;
pub mod values;
pub mod vault;
pub use groth16::EGRESS_KEY;
pub const USDC: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const ATA_PROGRAM: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const MAX_VALUES: usize = 2048;
const LIGHT: &[u8] = b"egress-light";
const CHECK: &[u8] = b"egress-checkpoint";
const BURN: &[u8] = b"egress-burn";

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Error {
    Encoding = 1,
    Binding,
    Proof,
    StaleCheckpoint,
    UnknownCheckpoint,
    Replay,
    Paused,
    WrongDestination,
}
impl From<Error> for ProgramError {
    fn from(error: Error) -> Self {
        ProgramError::Custom(error as u32)
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Bootstrap {
    pub vault_program: [u8; 32],
    pub ingress_policy: [u8; 48],
    pub ingress_epoch: u32,
    pub chain: [u8; 32],
    pub route: [u8; 32],
    pub solana_genesis: [u8; 32],
    pub pause_authority: [u8; 32],
    pub initial_checkpoint: [u8; 48],
    pub initial_committee: [u8; 48],
    pub initial_height: u64,
}
impl Bootstrap {
    /// Same commitment as postfiat_types::vault_bridge_route_binding.
    pub fn vault_route_binding(&self) -> [u8; 32] {
        // The WAN devnet's mainnet Solana ingress profile pins this commitment
        // at vault initialization, before the governance profile hash exists.
        if self.solana_genesis == hex_literal::hex!("45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0") {
            return solana_sha256_hasher::hashv(&[
                b"postfiat.pfusdc.solana.route_binding.v1",
                b"postfiat-wan-devnet-2",
                &hex_literal::hex!("ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9"),
                b"solana-mainnet-usdc-v1",
                &self.solana_genesis,
                &self.vault_program,
            ]).to_bytes();
        }
        let mut hash = Keccak256::new();
        hash.update(b"postfiat.vault_bridge.route_binding.v1\0");
        hash.update(self.ingress_policy);
        hash.update(self.ingress_epoch.to_be_bytes());
        hash.finalize().into()
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Light {
    pub vault_program: [u8; 32],
    pub vault_route_binding: [u8; 32],
    pub vault_alias: [u8; 20],
    pub mint_alias: [u8; 20],
    pub chain: [u8; 32],
    pub route: [u8; 32],
    pub solana_genesis: [u8; 32],
    pub pause_authority: [u8; 32],
    pub latest: [u8; 48],
    pub height: u64,
    pub paused: bool,
}

/// Preimage of a light's route: the exit descriptor (fields 6, 7), the family (23), the
/// first release's source bucket (26), the chain tag (32) and the vault and token code
/// hashes (34, 36). The vault and mint aliases (33, 35) are the light's own.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct RouteFields {
    pub exit_profile: [u8; 48],
    pub exit_epoch: [u8; 8],
    pub family: [u8; 48],
    pub bucket: [u8; 48],
    pub chain_tag: [u8; 8],
    pub vault_code: [u8; 32],
    pub token_code: [u8; 32],
}

/// The custody route that replaces a light's one-bucket route. The fields must reproduce
/// the stored route with field 26 in place, so the new route is fully determined by the
/// account, and a light that already holds the custody route is refused.
pub fn migrated_route(light: &Light, f: &RouteFields) -> Result<[u8; 32], ProgramError> {
    let old = solana_sha256_hasher::hashv(&[
        &f.exit_profile,
        &f.exit_epoch,
        &f.family,
        &f.bucket,
        &f.chain_tag,
        &light.vault_alias,
        &f.vault_code,
        &light.mint_alias,
        &f.token_code,
    ])
    .to_bytes();
    if old != light.route || &f.chain_tag != b"SOLANA01" {
        return Err(Error::Binding.into());
    }
    Ok(values::custody_route([
        &f.exit_profile,
        &f.exit_epoch,
        &f.family,
        &f.chain_tag,
        &light.vault_alias,
        &f.vault_code,
        &light.mint_alias,
        &f.token_code,
    ]))
}

/// The program's upgrade authority, read from its ProgramData account.
fn upgrade_authority(id: &Pubkey, programdata: &AccountInfo) -> Result<Pubkey, ProgramError> {
    let loader = solana_sdk_ids::bpf_loader_upgradeable::ID;
    let (key, _) = Pubkey::find_program_address(&[id.as_ref()], &loader);
    if programdata.key != &key || programdata.owner != &loader {
        return Err(ProgramError::InvalidSeeds);
    }
    // UpgradeableLoaderState::ProgramData: u32 tag 3, u64 slot, Option<Pubkey>.
    let data = programdata.try_borrow_data()?;
    match (data.get(..4), data.get(12), data.get(13..45)) {
        (Some(tag), Some(1), Some(authority)) if tag == 3u32.to_le_bytes() => {
            Ok(Pubkey::new_from_array(authority.try_into().unwrap()))
        }
        _ => Err(ProgramError::MissingRequiredSignature),
    }
}

fn pda(id: &Pubkey, account: &AccountInfo, seeds: &[&[u8]]) -> Result<u8, ProgramError> {
    let (key, bump) = Pubkey::find_program_address(seeds, id);
    if account.key != &key {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(bump)
}

// Transfer only the missing rent, then allocate and assign. Anyone may prefund a PDA.
fn create<'a>(
    id: &Pubkey,
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    sys: &AccountInfo<'a>,
    seeds: &[&[u8]],
    contents: &[u8],
) -> ProgramResult {
    if sys.key != &system_program::ID || account.owner != sys.key || !account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    let rent = Rent::get()?
        .minimum_balance(contents.len())
        .saturating_sub(account.lamports());
    if rent != 0 {
        invoke(
            &system::transfer(payer.key, account.key, rent),
            &[payer.clone(), account.clone(), sys.clone()],
        )?;
    }
    let infos = [account.clone(), sys.clone()];
    invoke_signed(
        &system::allocate(account.key, contents.len() as u64),
        &infos,
        &[seeds],
    )?;
    invoke_signed(&system::assign(account.key, id), &infos, &[seeds])?;
    account.try_borrow_mut_data()?.copy_from_slice(contents);
    Ok(())
}

fn read_light(id: &Pubkey, account: &AccountInfo) -> Result<Light, ProgramError> {
    pda(id, account, &[LIGHT])?;
    if account.owner != id {
        return Err(ProgramError::IllegalOwner);
    }
    Light::try_from_slice(&account.try_borrow_data()?).map_err(|_| ProgramError::InvalidAccountData)
}

fn save(light: &Light, account: &AccountInfo) -> ProgramResult {
    light
        .serialize(&mut &mut account.try_borrow_mut_data()?[..])
        .map_err(|_| ProgramError::InvalidAccountData)
}

fn initialize(id: &Pubkey, accounts: &[AccountInfo], args: &[u8]) -> ProgramResult {
    let [payer, deployment, light, initial, sys] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if deployment.key != id || !deployment.is_signer || !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let c = Bootstrap::try_from_slice(args).map_err(|_| Error::Encoding)?;
    if c.ingress_epoch == 0
        || c.initial_height == 0
        || c.initial_checkpoint == [0; 48]
        || c.initial_committee == [0; 48]
        || [
            c.vault_program,
            c.chain,
            c.route,
            c.solana_genesis,
            c.pause_authority,
        ]
        .contains(&[0; 32])
    {
        return Err(Error::Binding.into());
    }
    let bump = pda(id, light, &[LIGHT])?;
    let state = Light {
        vault_program: c.vault_program,
        vault_route_binding: c.vault_route_binding(),
        vault_alias: packet_alias(&Pubkey::new_from_array(c.vault_program)),
        mint_alias: packet_alias(&USDC),
        chain: c.chain,
        route: c.route,
        solana_genesis: c.solana_genesis,
        pause_authority: c.pause_authority,
        latest: c.initial_checkpoint,
        height: c.initial_height,
        paused: false,
    };
    create(
        id,
        payer,
        light,
        sys,
        &[LIGHT, &[bump]],
        &borsh::to_vec(&state).map_err(|_| Error::Encoding)?,
    )?;
    let digest = hash(&c.initial_checkpoint);
    let bump = pda(id, initial, &[CHECK, digest.as_ref()])?;
    create(
        id,
        payer,
        initial,
        sys,
        &[CHECK, digest.as_ref(), &[bump]],
        &c.initial_committee,
    )
}

fn packet_alias(key: &Pubkey) -> [u8; 20] {
    let mut h = Sha3_384::new();
    h.update(b"postfiat.solana.packet_alias.v1\0");
    h.update(hex::encode(key.as_ref()).as_bytes());
    h.finalize()[..20].try_into().unwrap()
}

fn destination_hash(genesis: &[u8; 32], owner: &Pubkey) -> [u8; 48] {
    let destination = format!(
        "solana-spl:{}:{}",
        hex::encode(genesis),
        hex::encode(owner.as_ref())
    );
    let mut h = Sha3_384::new();
    h.update(b"postfiat.vault_bridge.destination_ref_hash.v1\0");
    h.update(
        format!(
            "destination_ref_bytes={}\ndestination_ref={destination}\n",
            destination.len()
        )
        .as_bytes(),
    );
    h.finalize().into()
}

fn consume(id: &Pubkey, accounts: &[AccountInfo], proof: &[u8], withdraw: bool) -> ProgramResult {
    let [payer, light_account, prior, result, buffer, sys, rest @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !payer.is_signer || sys.key != &system_program::ID {
        return Err(ProgramError::InvalidArgument);
    }
    let mut light = read_light(id, light_account)?;
    if withdraw && light.paused {
        return Err(Error::Paused.into());
    }
    if buffer.data_len() > MAX_VALUES {
        return Err(Error::Encoding.into());
    }
    let data = buffer.try_borrow_data()?;
    let v = values::decode(&data, withdraw)?;
    if v.chain != light.chain {
        return Err(Error::Binding.into());
    }
    pda(id, prior, &[CHECK, hash(v.prior).as_ref()])?;
    if prior.owner != id || *prior.try_borrow_data()? != v.start_committee {
        return Err(Error::UnknownCheckpoint.into());
    }
    if !withdraw && (v.prior != light.latest || v.height <= light.height) {
        return Err(Error::StaleCheckpoint.into());
    }
    let digest = hash(v.result);
    let result_bump = pda(id, result, &[CHECK, digest.as_ref()])?;
    if result.owner == id && *result.try_borrow_data()? != v.committee {
        return Err(Error::Binding.into());
    }
    groth16::verify(&data, proof)?;
    if result.owner != id {
        create(
            id,
            payer,
            result,
            sys,
            &[CHECK, digest.as_ref(), &[result_bump]],
            v.committee,
        )?;
    }
    if v.height > light.height {
        light.height = v.height;
        light.latest.copy_from_slice(v.result);
        save(&light, light_account)?;
    }
    match &v.exit {
        Some(exit) => release_burn(id, payer, sys, rest, &light, exit),
        None if rest.is_empty() => Ok(()),
        None => Err(ProgramError::InvalidArgument),
    }
}

fn release_burn<'a>(
    id: &Pubkey,
    payer: &AccountInfo<'a>,
    sys: &AccountInfo<'a>,
    rest: &[AccountInfo<'a>],
    light: &Light,
    exit: &values::Exit,
) -> ProgramResult {
    let [nullifier, vault_program, authority, config, custody, ata, owner, mint, token_program] =
        rest
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if exit.route != light.route
        || vault_program.key.as_ref() != light.vault_program
        || !vault_program.executable
        || mint.key != &USDC
        || token_program.key != &spl_token_interface::ID
        || light.vault_alias != exit.vault_alias
        || light.mint_alias != exit.mint_alias
    {
        return Err(Error::Binding.into());
    }
    if destination_hash(&light.solana_genesis, owner.key) != exit.destination_hash {
        return Err(Error::WrongDestination.into());
    }
    let auth_bump = pda(
        id,
        authority,
        &[vault::AUTHORITY, vault_program.key.as_ref()],
    )?;
    let burn = hash(exit.burn);
    let bump = pda(id, nullifier, &[BURN, burn.as_ref()])?;
    if nullifier.owner == id {
        return Err(Error::Replay.into());
    }
    create(
        id,
        payer,
        nullifier,
        sys,
        &[BURN, burn.as_ref(), &[bump]],
        &[1],
    )?;
    let accounts = vec![
        AccountMeta::new_readonly(*authority.key, true),
        AccountMeta::new_readonly(*config.key, false),
        AccountMeta::new(*custody.key, false),
        AccountMeta::new(*ata.key, false),
        AccountMeta::new_readonly(*owner.key, false),
        AccountMeta::new_readonly(*mint.key, false),
        AccountMeta::new_readonly(*token_program.key, false),
    ];
    invoke_signed(
        &Instruction {
            program_id: *vault_program.key,
            accounts,
            data: [
                &[2][..],
                &exit.amount.to_le_bytes(),
                &light.vault_route_binding,
            ]
            .concat(),
        },
        &[
            authority.clone(),
            config.clone(),
            custody.clone(),
            ata.clone(),
            owner.clone(),
            mint.clone(),
            token_program.clone(),
            vault_program.clone(),
        ],
        &[&[vault::AUTHORITY, vault_program.key.as_ref(), &[auth_bump]]],
    )?;
    Ok(())
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program_entrypoint::entrypoint!(process_instruction);

pub fn process_instruction(id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let (&tag, args) = data.split_first().ok_or(Error::Encoding)?;
    match tag {
        2 | 8 => consume(id, accounts, args, tag == 2),
        3 => initialize(id, accounts, args),
        4 => {
            let [admin, account] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            let mut light = read_light(id, account)?;
            if !admin.is_signer || admin.key.as_ref() != light.pause_authority {
                return Err(ProgramError::MissingRequiredSignature);
            }
            light.paused = match args {
                [0] => false,
                [1] => true,
                _ => return Err(Error::Encoding.into()),
            };
            save(&light, account)
        }
        // Verifier-owned upload buffers must be on curve and sign edits or reclamation.
        // Protocol state is exclusively stored in off-curve PDAs.
        5 | 6 => {
            let (buffer, rest) = accounts
                .split_first()
                .ok_or(ProgramError::NotEnoughAccountKeys)?;
            if !buffer.is_signer
                || buffer.owner != id
                || !buffer.key.is_on_curve()
                || buffer.data_len() > MAX_VALUES
            {
                return Err(ProgramError::InvalidAccountData);
            }
            if tag == 5 {
                let off =
                    u16::from_le_bytes(args.get(..2).ok_or(Error::Encoding)?.try_into().unwrap())
                        as usize;
                buffer
                    .try_borrow_mut_data()?
                    .get_mut(off..off + args.len() - 2)
                    .ok_or(Error::Encoding)?
                    .copy_from_slice(&args[2..]);
            } else {
                let [recipient] = rest else {
                    return Err(ProgramError::NotEnoughAccountKeys);
                };
                if !args.is_empty() || recipient.key == buffer.key {
                    return Err(ProgramError::InvalidArgument);
                }
                let sum = recipient
                    .lamports()
                    .checked_add(buffer.lamports())
                    .ok_or(ProgramError::ArithmeticOverflow)?;
                **recipient.try_borrow_mut_lamports()? = sum;
                **buffer.try_borrow_mut_lamports()? = 0;
                buffer.resize(0)?;
                buffer.assign(&system_program::ID);
            }
            Ok(())
        }
        7 => {
            let [buffer] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            if buffer.data_len() > MAX_VALUES {
                return Err(Error::Encoding.into());
            }
            groth16::verify(&buffer.try_borrow_data()?, args)
        }
        // One-time route migration of a light initialised by the first release, signed by
        // the upgrade authority right after the upgrade. Checkpoint, markers and the pause
        // state stay as they are; only `route` changes.
        9 => {
            let [authority, account, programdata] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            if !authority.is_signer || upgrade_authority(id, programdata)? != *authority.key {
                return Err(ProgramError::MissingRequiredSignature);
            }
            let fields = RouteFields::try_from_slice(args).map_err(|_| Error::Encoding)?;
            let mut light = read_light(id, account)?;
            light.route = migrated_route(&light, &fields)?;
            save(&light, account)
        }
        _ => Err(Error::Encoding.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex<const N: usize>(text: &str) -> [u8; N] {
        hex::decode(text).unwrap().try_into().unwrap()
    }

    /// The mainnet light (FQe34dCQg9m6yEW6NjbjoR8LfWwKGLX2hBvTdY2Lu6s1) as read at slot
    /// 473290673: bound to the revision-8 bucket, checkpoint at Post Fiat block 1851.
    fn mainnet_light() -> Light {
        Light::try_from_slice(&hex::decode(include_str!("../tests/mainnet-light.hex").trim()).unwrap())
            .unwrap()
    }

    fn mainnet_fields() -> RouteFields {
        let vector = include_str!("../tests/route-vector.txt");
        let value = |key: &str| {
            vector
                .lines()
                .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
                .unwrap()
        };
        RouteFields {
            exit_profile: unhex(value("f6")),
            exit_epoch: unhex(value("f7")),
            family: unhex(value("f23")),
            // The first release's revision-8 bucket (postfiat_types::solana_exit::BUCKET).
            bucket: unhex("4c045e289eaaa2c7ae7592b96ec0eeabe9fa0cfdd032783d359409dcc08d9dd0dfdd7cc8d483d6a88c12dc713522d2cc"),
            chain_tag: unhex(value("f32")),
            vault_code: unhex(value("f34")),
            token_code: unhex(value("f36")),
        }
    }

    #[test]
    fn mainnet_light_migrates_to_the_custody_route_and_keeps_everything_else() {
        let light = mainnet_light();
        assert_eq!(light.height, 1851);
        let route = migrated_route(&light, &mainnet_fields()).unwrap();
        // The same route a fresh initialisation of this branch writes (tests/route-vector.txt).
        assert_eq!(hex::encode(route), "02b02d4b458283c0e351a34adf9e25c896cca0cd9f2094d00a50e7f92a4d558e");
        // Only bytes 136..168 (route) of the 289-byte account change.
        let before = borsh::to_vec(&light).unwrap();
        let mut after = mainnet_light();
        after.route = route;
        let after = borsh::to_vec(&after).unwrap();
        assert_eq!(before.len(), 289);
        let changed: Vec<usize> = (0..289).filter(|&i| before[i] != after[i]).collect();
        assert!(changed.iter().all(|i| (136..168).contains(i)) && !changed.is_empty());
    }

    #[test]
    fn migration_runs_once_and_only_from_the_exact_old_route() {
        let mut light = mainnet_light();
        light.route = migrated_route(&light, &mainnet_fields()).unwrap();
        // A second run finds the custody route, not the one-bucket route.
        assert!(migrated_route(&light, &mainnet_fields()).is_err());
        let light = mainnet_light();
        let mut other = mainnet_fields();
        other.bucket[0] ^= 1;
        assert!(migrated_route(&light, &other).is_err());
        let mut other = mainnet_fields();
        other.vault_code[0] ^= 1;
        assert!(migrated_route(&light, &other).is_err());
    }
}
