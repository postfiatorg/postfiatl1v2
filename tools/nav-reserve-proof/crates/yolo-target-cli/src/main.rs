use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use reserve_proof_types::yolo_target::{
    create_yolo_portfolio_target_v1, YoloPortfolioParametersV1, YoloPortfolioTargetInputV1,
};
use sha2::{Digest, Sha256};

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
}

fn main() -> Result<()> {
    match Args::parse().command {
        Command::Calculate {
            methodology,
            parameters,
            input,
            output,
        } => calculate(methodology, parameters, input, output),
    }
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
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "{label} is not a regular file: {}",
        path.display()
    );
    let length = usize::try_from(metadata.len()).context("input length exceeds platform usize")?;
    anyhow::ensure!(length <= maximum, "{label} exceeds {maximum} bytes");
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
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
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create new output {}", path.display()))?;
    output.write_all(bytes)?;
    output.sync_all()?;
    Ok(())
}
