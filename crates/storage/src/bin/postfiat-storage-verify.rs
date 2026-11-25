//! Operator maintenance for one existing transactional data directory.
use postfiat_storage::transactional::verify_existing_generation;
use std::{env, error::Error, path::PathBuf, process::ExitCode};

const USAGE: &str =
    "Usage: postfiat-storage-verify --data-dir PATH --chain-id ID --genesis-hash HEX";

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let (mut data_dir, mut chain_id, mut genesis_hash) = (None, None, None);
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--help" | "-h") => {
                println!("{USAGE}");
                return Ok(());
            }
            Some("--data-dir") if data_dir.is_none() => {
                data_dir = Some(PathBuf::from(args.next().ok_or(USAGE)?));
            }
            Some("--chain-id") if chain_id.is_none() => {
                chain_id = Some(
                    args.next()
                        .and_then(|v| v.into_string().ok())
                        .ok_or(USAGE)?,
                );
            }
            Some("--genesis-hash") if genesis_hash.is_none() => {
                genesis_hash = Some(
                    args.next()
                        .and_then(|v| v.into_string().ok())
                        .ok_or(USAGE)?,
                );
            }
            _ => return Err(USAGE.into()),
        }
    }
    let hash = genesis_hash.ok_or(USAGE)?;
    if hash.len() != 96
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected 96 lowercase hexadecimal characters for the genesis hash".into());
    }
    let report =
        verify_existing_generation(&data_dir.ok_or(USAGE)?, &chain_id.ok_or(USAGE)?, &hash)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("postfiat-storage-verify: {error}");
            ExitCode::FAILURE
        }
    }
}
