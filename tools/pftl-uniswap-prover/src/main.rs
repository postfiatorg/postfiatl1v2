use std::{fs, path::PathBuf, time::Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use postfiat_pftl_uniswap_proofs::{
    verify_pftl_uniswap_checkpoint_witness_v1, verify_pftl_uniswap_receipt_witness_v1,
    PftlUniswapCheckpointProofWitnessV1, PftlUniswapProofInputV1, PftlUniswapReceiptProofWitnessV1,
};
use sha2::{Digest, Sha256};
use sp1_sdk::{Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Stdin};

const RECEIPT_ELF_BYTES: &[u8] =
    include_bytes!("../../../programs/pftl-uniswap-receipt/elf/pftl-uniswap-receipt-program");
const RECEIPT_ELF: Elf = Elf::Static(RECEIPT_ELF_BYTES);

#[derive(Debug, Parser)]
#[command(name = "pftl-uniswap-prover")]
#[command(about = "Execute or Groth16-prove a666 PFTL finality witnesses")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Derive the immutable guest ELF hash and SP1 program verification key.
    ProgramInfo {
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Execute or Groth16-prove one finalized a666 export receipt.
    Receipt {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a receipt-independent checkpoint segment.
    Checkpoint {
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
    match Args::parse().command {
        Command::ProgramInfo { output } => program_info(output).await,
        Command::Receipt {
            witness,
            output_dir,
            prove,
        } => prove_receipt(witness, output_dir, prove).await,
        Command::Checkpoint {
            witness,
            output_dir,
            prove,
        } => prove_checkpoint(witness, output_dir, prove).await,
    }
}

async fn program_info(output: Option<PathBuf>) -> Result<()> {
    let client = ProverClient::from_env().await;
    let proving_key = client.setup(RECEIPT_ELF).await?;
    let report = serde_json::json!({
        "schema": "postfiat-pftl-uniswap-program-info-v1",
        "elf_sha256": hex::encode(Sha256::digest(RECEIPT_ELF_BYTES)),
        "program_vkey": proving_key.verifying_key().bytes32(),
    });
    let encoded = serde_json::to_vec_pretty(&report)?;
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, [&encoded[..], b"\n"].concat())?;
    }
    println!("{}", String::from_utf8(encoded)?);
    Ok(())
}

async fn prove_receipt(witness_path: PathBuf, output_dir: PathBuf, prove: bool) -> Result<()> {
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read receipt witness {}", witness_path.display()))?;
    let witness: PftlUniswapReceiptProofWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode receipt witness {}", witness_path.display()))?;
    let expected = verify_pftl_uniswap_receipt_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native receipt verification failed: {error}"))?
        .abi_encode();
    execute_or_prove(
        witness_path,
        output_dir,
        prove,
        PftlUniswapProofInputV1::Receipt(Box::new(witness)),
        expected,
        "receipt",
    )
    .await
}

async fn prove_checkpoint(witness_path: PathBuf, output_dir: PathBuf, prove: bool) -> Result<()> {
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read checkpoint witness {}", witness_path.display()))?;
    let witness: PftlUniswapCheckpointProofWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode checkpoint witness {}", witness_path.display()))?;
    let expected = verify_pftl_uniswap_checkpoint_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native checkpoint verification failed: {error}"))?
        .abi_encode();
    execute_or_prove(
        witness_path,
        output_dir,
        prove,
        PftlUniswapProofInputV1::Checkpoint(Box::new(witness)),
        expected,
        "checkpoint",
    )
    .await
}

async fn execute_or_prove(
    witness_path: PathBuf,
    output_dir: PathBuf,
    prove: bool,
    input: PftlUniswapProofInputV1,
    expected_public_values: Vec<u8>,
    proof_kind: &str,
) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(serde_cbor::to_vec(&input).context("encode witness as versioned CBOR")?);
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (public_values, report) = client.execute(RECEIPT_ELF, stdin.clone()).await?;
    let executed = public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 {proof_kind} output differs from native canonical public values"
    );

    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat-pftl-uniswap-execute-report-v1",
            "proof_kind": proof_kind,
            "witness": witness_path,
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;

    if prove {
        let prove_started = Instant::now();
        let proving_key = client.setup(RECEIPT_ELF).await?;
        let proof = client.prove(&proving_key, stdin).groth16().await?;
        client.verify(&proof, proving_key.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified Groth16 proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
        fs::write(
            output_dir.join("proof-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "postfiat-pftl-uniswap-proof-report-v1",
                "proof_kind": proof_kind,
                "program_vkey": proving_key.verifying_key().bytes32(),
                "proof_mode": "groth16",
                "setup_and_prove_ms": prove_started.elapsed().as_millis(),
                "proof_bytes": proof.bytes().len(),
                "public_values_bytes": proof.public_values.to_vec().len(),
            }))?,
        )?;
    }
    println!(
        "{proof_kind} witness executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    Ok(())
}
