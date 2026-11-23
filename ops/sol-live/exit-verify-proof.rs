// Feed the deployed verifier's read-only tag 7 precisely the worker's proof and values.
use pfusdc_egress_verifier::{process_instruction, MAX_VALUES};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin().take((357 + MAX_VALUES) as u64).read_to_end(&mut input)?;
    if !(357..=356 + MAX_VALUES).contains(&input.len()) {
        return Err("invalid proof or values length".into());
    }
    let (proof, values) = input.split_at_mut(356);
    let address = Pubkey::default();
    let mut lamports = 0;
    let buffer = AccountInfo::new(&address, false, false, &mut lamports, values, &address, false);
    let instruction = [&[7u8][..], proof].concat();
    process_instruction(&address, &[buffer], &instruction)?;
    Ok(())
}
