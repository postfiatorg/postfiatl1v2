//! Executes the lane export program on a proof input in SP1's CPU executor
//! and checks the committed bytes against the statement run on the host. No
//! proof is generated and no prover network is used.

use std::{fs, path::PathBuf, time::Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use postfiat_pft_lane_proofs::{
    verify_external_lane_checkpoint_witness_v1, verify_external_lane_export_witness_v1,
};
use postfiat_types::ExternalLaneExportProgramInputV1;
use sp1_sdk::{Elf, Prover, ProverClient, SP1Stdin};

#[derive(Debug, Parser)]
#[command(name = "pft-lane-export-prover")]
#[command(about = "Execute the PFT lane export program on a proof input")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Execute (no proving) and compare the committed public values.
    Execute {
        #[arg(long)]
        elf: PathBuf,
        /// `input.cbor` from `postfiat-node external-lane-export-proof-input`.
        #[arg(
            long,
            conflicts_with = "input_json",
            required_unless_present = "input_json"
        )]
        input: Option<PathBuf>,
        /// The same input as JSON (`{"kind": "checkpoint", "witness": ...}`),
        /// encoded to the versioned CBOR before execution.
        #[arg(long)]
        input_json: Option<PathBuf>,
        #[arg(long)]
        output_dir: PathBuf,
    },
}

fn expected_public_values(input: &[u8]) -> Result<Vec<u8>> {
    let decoded: ExternalLaneExportProgramInputV1 =
        serde_cbor::from_slice(input).context("decode the versioned CBOR input")?;
    let bytes = match decoded {
        ExternalLaneExportProgramInputV1::Withdrawal(witness) => {
            verify_external_lane_export_witness_v1(&witness)
                .map_err(anyhow::Error::msg)?
                .canonical_bytes_without_commitment()
        }
        ExternalLaneExportProgramInputV1::Checkpoint(witness) => {
            verify_external_lane_checkpoint_witness_v1(&witness)
                .map_err(anyhow::Error::msg)?
                .canonical_bytes_without_commitment()
        }
    };
    bytes.map_err(anyhow::Error::msg)
}

#[tokio::main]
async fn main() -> Result<()> {
    let Command::Execute {
        elf,
        input,
        input_json,
        output_dir,
    } = Args::parse().command;
    anyhow::ensure!(
        std::env::var("SP1_PROVER").as_deref() == Ok("cpu"),
        "set SP1_PROVER=cpu: this tool only executes locally"
    );
    let elf = Elf::from(fs::read(&elf).with_context(|| format!("read {}", elf.display()))?);
    let input = match (input, input_json) {
        (Some(path), _) => fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        (None, Some(path)) => {
            let decoded: ExternalLaneExportProgramInputV1 =
                serde_json::from_slice(&fs::read(&path)?).context("decode the JSON input")?;
            serde_cbor::to_vec(&decoded).context("encode the CBOR input")?
        }
        (None, None) => anyhow::bail!("name --input or --input-json"),
    };
    let expected = expected_public_values(&input)?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(input);
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (public_values, report) = client.execute(elf, stdin).await?;
    let executed = public_values.to_vec();
    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    let report = serde_json::json!({
        "schema": "postfiat-pft-lane-export-execute-report-v1",
        "matches_host_statement": executed == expected,
        "instruction_count": report.total_instruction_count(),
        "elapsed_ms": started.elapsed().as_millis(),
        "public_values_bytes": executed.len(),
    });
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    anyhow::ensure!(
        executed == expected,
        "the guest committed other bytes than the host statement"
    );
    Ok(())
}
