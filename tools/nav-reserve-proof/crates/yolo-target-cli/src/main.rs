use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use postfiat_types::YoloTargetPublicValuesV1;
use reserve_proof_types::yolo_target::{
    create_yolo_portfolio_target_v1, YoloPortfolioParametersV1, YoloPortfolioTargetInputV1,
};
use reserve_proof_types::yolo_target_proof::execute_yolo_target_proof;
use reserve_proof_types::yolo_witness::{
    decode_target_witness, TargetAcceptanceV1, TargetProofWitnessV1, MAX_TARGET_WITNESS_BYTES,
};
use sha2::{Digest, Sha256};

#[cfg(feature = "sp1")]
mod sp1;

const MAX_TARGET_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_METHODOLOGY_BYTES: usize = 4 * 1024 * 1024;
const MAX_PARAMETER_BYTES: usize = 64 * 1024;

#[derive(Debug, Parser)]
#[command(name = "postfiat-yolo-target")]
#[command(about = "Calculate a deterministic YOLO portfolio target without placing orders")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate the locked methodology inputs and write the deterministic target.
    Calculate {
        #[arg(long)]
        methodology: PathBuf,
        #[arg(long)]
        parameters: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Validate signed private JSON under independently approved public pins.
    CreateWitness {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        acceptance: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify Nitro, provenance and calculation natively; export only public values.
    NativeExecute {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        acceptance: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Decode the exact 408-byte target ABI.
    Decode {
        #[arg(long)]
        public_values: PathBuf,
    },
    #[cfg(feature = "sp1")]
    /// Execute the pinned ELF locally and compare with native public values.
    GuestExecute {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        acceptance: PathBuf,
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    #[cfg(feature = "sp1")]
    /// Generate and verify a local CPU Groth16 proof (release build required).
    Prove {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        acceptance: PathBuf,
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
    },
    #[cfg(feature = "sp1")]
    /// Verify a Groth16 proof against a pinned ELF and approved public manifest.
    Verify {
        #[arg(long)]
        proof: PathBuf,
        #[arg(long)]
        acceptance: PathBuf,
        #[arg(long)]
        elf: PathBuf,
    },
    #[cfg(feature = "sp1")]
    /// Report the ELF SHA-256 and derived SP1 program verification key.
    Identity {
        #[arg(long)]
        elf: PathBuf,
    },
}

fn main() -> Result<()> {
    match Args::parse().command {
        Command::Calculate {
            methodology,
            parameters,
            input,
            output,
        } => calculate(methodology, parameters, input, output),
        Command::CreateWitness {
            input,
            acceptance,
            output,
        } => {
            let input = read_bounded(&input, MAX_TARGET_WITNESS_BYTES, "private witness JSON")?;
            let witness: TargetProofWitnessV1 =
                serde_json::from_slice(&input).context("decode private witness JSON")?;
            let encoded = serde_cbor::to_vec(&witness)?;
            let (public, _) = accepted_native(&encoded, &acceptance)?;
            write_new(&output, &encoded)?;
            report(
                &serde_json::json!({"schema": "postfiat.yolo.witness_construction.v1", "witnessBytes": encoded.len(), "publicValues": public}),
            )
        }
        Command::NativeExecute {
            witness,
            acceptance,
            output,
        } => {
            let encoded = read_bounded(&witness, MAX_TARGET_WITNESS_BYTES, "private witness CBOR")?;
            let (public, _) = accepted_native(&encoded, &acceptance)?;
            write_new(&output, &public.encode().map_err(anyhow::Error::msg)?)?;
            report(
                &serde_json::json!({"schema": "postfiat.yolo.native_execution.v1", "publicValues": public}),
            )
        }
        Command::Decode { public_values } => {
            let bytes = read_bounded(&public_values, 408, "target public values")?;
            let public = YoloTargetPublicValuesV1::decode(&bytes).map_err(anyhow::Error::msg)?;
            report(&serde_json::to_value(public)?)
        }
        #[cfg(feature = "sp1")]
        Command::GuestExecute {
            witness,
            acceptance,
            elf,
            output,
        } => sp1::execute(witness, acceptance, elf, output),
        #[cfg(feature = "sp1")]
        Command::Prove {
            witness,
            acceptance,
            elf,
            output_dir,
        } => sp1::prove(witness, acceptance, elf, output_dir),
        #[cfg(feature = "sp1")]
        Command::Verify {
            proof,
            acceptance,
            elf,
        } => sp1::verify(proof, acceptance, elf),
        #[cfg(feature = "sp1")]
        Command::Identity { elf } => sp1::identity(elf),
    }
}

fn report(value: &serde_json::Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn acceptance(path: &PathBuf) -> Result<TargetAcceptanceV1> {
    serde_json::from_slice(&read_bounded(
        path,
        4096,
        "independently approved public pins",
    )?)
    .context("decode public acceptance pins")
}

fn accepted_native(
    encoded: &[u8],
    path: &PathBuf,
) -> Result<(YoloTargetPublicValuesV1, TargetAcceptanceV1)> {
    let pins = acceptance(path)?;
    let witness = decode_target_witness(encoded).map_err(anyhow::Error::msg)?;
    let public = execute_yolo_target_proof(&witness).map_err(anyhow::Error::msg)?;
    pins.verify(&public).map_err(anyhow::Error::msg)?;
    Ok((public, pins))
}

fn calculate(
    methodology_path: PathBuf,
    parameters_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<()> {
    let methodology = read_bounded(&methodology_path, MAX_METHODOLOGY_BYTES, "YOLO methodology")?;
    let parameters: YoloPortfolioParametersV1 = serde_json::from_slice(&read_bounded(
        &parameters_path,
        MAX_PARAMETER_BYTES,
        "YOLO parameter manifest",
    )?)
    .with_context(|| format!("decode JSON {}", parameters_path.display()))?;
    let input: YoloPortfolioTargetInputV1 = serde_json::from_slice(&read_bounded(
        &input_path,
        MAX_TARGET_INPUT_BYTES,
        "YOLO portfolio target input",
    )?)
    .with_context(|| format!("decode JSON {}", input_path.display()))?;
    let methodology_sha256 = hex::encode(Sha256::digest(methodology));
    anyhow::ensure!(
        methodology_sha256 == input.methodology_sha256,
        "portfolio input methodologySha256 does not match the methodology file"
    );
    let target =
        create_yolo_portfolio_target_v1(&input, &parameters).map_err(anyhow::Error::msg)?;
    let target_sha256 = target.sha256().map_err(anyhow::Error::msg)?;
    let mut output_value = serde_json::to_value(&target)?;
    output_value
        .as_object_mut()
        .context("YOLO target did not serialize as an object")?
        .insert(
            "targetSha256".to_string(),
            serde_json::Value::String(target_sha256.clone()),
        );
    let mut encoded = serde_json::to_vec_pretty(&output_value)?;
    encoded.push(b'\n');
    write_new(&output_path, &encoded)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.yolo.target_calculation.v1",
            "output": output_path,
            "targetSha256": target_sha256,
            "status": target.status,
        }))?
    );
    Ok(())
}

fn read_bounded(path: &PathBuf, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("stat {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "{label} is not a regular file: {}",
        path.display()
    );
    let length = usize::try_from(metadata.len()).context("input length exceeds platform usize")?;
    anyhow::ensure!(length <= maximum, "{label} exceeds {maximum} bytes");
    let mut bytes = Vec::with_capacity(length);
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", path.display()))?;
    anyhow::ensure!(
        bytes.len() == length,
        "{label} changed while being read: {}",
        path.display()
    );
    Ok(bytes)
}

fn write_new(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // The same writer handles private witnesses and unsealed target files.
        options.mode(0o600);
    }
    let mut output = options
        .open(path)
        .with_context(|| format!("create new output {}", path.display()))?;
    output.write_all(bytes)?;
    output.sync_all()?;
    Ok(())
}
