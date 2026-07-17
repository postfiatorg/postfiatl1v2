use std::{fs, path::PathBuf, time::Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use postfiat_pfusdc_proofs::verify_egress_witness_v1;
use postfiat_types::PfUsdcEgressProofWitnessV1;
use sha3::{Digest, Sha3_384};
use sp1_sdk::{Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Stdin};

const EGRESS_ELF: Elf = Elf::Static(include_bytes!(
    "../../../programs/pfusdc-egress/elf/pfusdc-egress-program"
));

#[derive(Debug, Parser)]
#[command(name = "pfusdc-tier4-prover")]
#[command(about = "Proof builder for the proof-native pfUSDC Tier-4 route")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Execute or Groth16-prove a canonical PFTL egress witness.
    Egress {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    sp1_sdk::utils::setup_logger();
    let args = Args::parse();
    match args.command {
        Command::Egress {
            witness,
            output_dir,
            prove,
        } => prove_egress(witness, output_dir, prove).await,
    }
}

async fn prove_egress(witness_path: PathBuf, output_dir: PathBuf, prove: bool) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read egress witness {}", witness_path.display()))?;
    let witness: PfUsdcEgressProofWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode egress witness {}", witness_path.display()))?;
    let expected = verify_egress_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native egress witness verification failed: {error}"))?;
    let expected_public_values = expected
        .canonical_bytes_without_commitment()
        .map_err(|error| anyhow::anyhow!("encode expected public values: {error}"))?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(EGRESS_ELF, stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 egress output differs from native canonical public values"
    );
    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    fs::write(
        output_dir.join("public-values.sha3-384"),
        hex::encode(Sha3_384::digest(&executed)),
    )?;
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat.pfusdc.egress_execute_report.v1",
            "witness": witness_path,
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    println!(
        "egress witness executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    if prove {
        let setup_started = Instant::now();
        let pk = client.setup(EGRESS_ELF).await?;
        let proof = client.prove(&pk, stdin).groth16().await?;
        client.verify(&proof, pk.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified Groth16 proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
        fs::write(
            output_dir.join("proof-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "postfiat.pfusdc.egress_proof_report.v1",
                "program_vkey": pk.verifying_key().bytes32(),
                "setup_and_prove_ms": setup_started.elapsed().as_millis(),
                "proof_bytes": proof.bytes().len(),
                "public_values_bytes": proof.public_values.to_vec().len(),
            }))?,
        )?;
        println!(
            "verified Groth16 proof; vkey {}",
            pk.verifying_key().bytes32()
        );
    }
    Ok(())
}
