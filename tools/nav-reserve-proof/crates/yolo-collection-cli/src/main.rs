use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use postfiat_types::{YoloCollectionPublicValuesV1, YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES};
use reserve_proof_types::yolo_collection::{
    execute_yolo_collection_proof, YoloCollectionProofWitnessV1,
    YOLO_MAX_COLLECTION_WITNESS_BYTES_V1,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "postfiat-yolo-collection")]
#[command(about = "Build and inspect disclosure-safe YOLO collection proof inputs")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate reviewed JSON and encode bounded CBOR for the SP1 guest.
    WitnessBuild {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Execute the same deterministic logic as the SP1 guest without proving.
    Execute {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Decode and validate the fixed-width PFTL public-values ABI.
    Decode {
        #[arg(long)]
        public_values: PathBuf,
    },
}

fn main() -> Result<()> {
    match Args::parse().command {
        Command::WitnessBuild { input, output } => witness_build(input, output),
        Command::Execute { witness, output } => execute(witness, output),
        Command::Decode { public_values } => decode(public_values),
    }
}

fn witness_build(input: PathBuf, output: PathBuf) -> Result<()> {
    let witness: YoloCollectionProofWitnessV1 = serde_json::from_slice(&read_bounded(
        &input,
        YOLO_MAX_COLLECTION_WITNESS_BYTES_V1,
        "YOLO collection witness JSON",
    )?)
    .with_context(|| format!("decode JSON {}", input.display()))?;
    let expected = execute_yolo_collection_proof(&witness).map_err(anyhow::Error::msg)?;
    let encoded = encode_human_readable_cbor(&witness)?;
    anyhow::ensure!(
        encoded.len() <= YOLO_MAX_COLLECTION_WITNESS_BYTES_V1,
        "YOLO collection witness exceeds its bounded maximum"
    );
    let decoded: YoloCollectionProofWitnessV1 = serde_cbor::from_slice(&encoded)?;
    anyhow::ensure!(
        execute_yolo_collection_proof(&decoded).map_err(anyhow::Error::msg)? == expected,
        "CBOR round trip changed YOLO collection public values"
    );
    write_new(&output, &encoded)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.yolo.collection_witness_build.v1",
            "witness": output,
            "epochSha256": expected.epoch_sha256,
            "commitmentsSha256": expected.commitments_sha256,
            "snapshotCount": expected.snapshot_count,
        }))?
    );
    Ok(())
}

fn execute(witness_path: PathBuf, output: PathBuf) -> Result<()> {
    let encoded = read_bounded(
        &witness_path,
        YOLO_MAX_COLLECTION_WITNESS_BYTES_V1,
        "YOLO collection witness",
    )?;
    let witness: YoloCollectionProofWitnessV1 = serde_cbor::from_slice(&encoded)?;
    let values = execute_yolo_collection_proof(&witness).map_err(anyhow::Error::msg)?;
    let public_values = values.encode().map_err(anyhow::Error::msg)?;
    write_new(&output, &public_values)?;
    println!("{}", serde_json::to_string_pretty(&values)?);
    Ok(())
}

fn decode(path: PathBuf) -> Result<()> {
    let encoded = read_bounded(
        &path,
        YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES,
        "YOLO collection public values",
    )?;
    let values = YoloCollectionPublicValuesV1::decode(&encoded).map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        values.encode().map_err(anyhow::Error::msg)? == encoded,
        "YOLO collection public values are not canonical"
    );
    println!("{}", serde_json::to_string_pretty(&values)?);
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

fn encode_human_readable_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    Ok(serde_cbor::to_vec(&serde_json::to_value(value)?)?)
}
