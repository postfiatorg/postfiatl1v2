use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use postfiat_types::{NavProfileRegisterOperation, NavReservePublicValuesV1};
use reserve_proof_types::b23_nav::{decode, execute, MAX_WITNESS};
use std::{fs, path::PathBuf};
#[cfg(feature = "sp1")]
mod sp1;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Native {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Profile {
        #[arg(long)]
        operation: PathBuf,
    },
    Decode {
        #[arg(long)]
        public_values: PathBuf,
    },
    #[cfg(feature = "sp1")]
    Identity {
        #[arg(long)]
        elf: PathBuf,
    },
    #[cfg(feature = "sp1")]
    Execute {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    #[cfg(feature = "sp1")]
    Prove {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    #[cfg(feature = "sp1")]
    Verify {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        proof: PathBuf,
    },
}
fn read(path: &PathBuf, max: usize) -> Result<Vec<u8>> {
    ensure!(
        fs::metadata(path)?.len() <= max as u64,
        "input exceeds bound"
    );
    let b = fs::read(path)?;
    ensure!(b.len() <= max, "input exceeds bound");
    Ok(b)
}
fn write(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
fn native(path: &PathBuf) -> Result<(Vec<u8>, Vec<u8>)> {
    let bytes = read(path, MAX_WITNESS)?;
    let values =
        execute(&decode(&bytes).map_err(anyhow::Error::msg)?).map_err(anyhow::Error::msg)?;
    Ok((bytes, values.encode().map_err(anyhow::Error::msg)?))
}
fn report(value: serde_json::Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
fn main() -> Result<()> {
    match Args::parse().command {
        Command::Native { witness, output } => {
            let (_, values) = native(&witness)?;
            write(&output, &values)?;
            report(serde_json::json!({"native_verified":true,"bytes":values.len()}))
        }
        Command::Profile { operation } => {
            let op: NavProfileRegisterOperation =
                serde_json::from_slice(&read(&operation, 65536)?)?;
            report(serde_json::to_value(
                op.to_profile().map_err(anyhow::Error::msg)?,
            )?)
        }
        Command::Decode { public_values } => report(serde_json::to_value(
            NavReservePublicValuesV1::decode(&read(&public_values, 584)?)
                .map_err(anyhow::Error::msg)?,
        )?),
        #[cfg(feature = "sp1")]
        Command::Identity { elf } => sp1::identity(elf),
        #[cfg(feature = "sp1")]
        Command::Execute {
            witness,
            elf,
            output,
        } => sp1::execute(witness, elf, output),
        #[cfg(feature = "sp1")]
        Command::Prove {
            witness,
            elf,
            output,
        } => sp1::prove(witness, elf, output),
        #[cfg(feature = "sp1")]
        Command::Verify {
            witness,
            elf,
            proof,
        } => sp1::verify(witness, elf, proof),
    }
}
