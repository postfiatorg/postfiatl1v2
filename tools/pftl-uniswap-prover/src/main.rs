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
        /// Override the embedded ELF, for reproducing an already-deployed program key.
        #[arg(long)]
        elf: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Execute or Groth16-prove one finalized a666 export receipt.
    Receipt {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        /// Override the embedded ELF, for reproducing an already-deployed program key.
        #[arg(long)]
        elf: Option<PathBuf>,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a receipt-independent checkpoint segment.
    Checkpoint {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        /// Override the embedded ELF, for reproducing an already-deployed program key.
        #[arg(long)]
        elf: Option<PathBuf>,
        #[arg(long)]
        prove: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    sp1_sdk::utils::setup_logger();
    match Args::parse().command {
        Command::ProgramInfo { elf, output } => program_info(elf, output).await,
        Command::Receipt {
            witness,
            output_dir,
            elf,
            prove,
        } => prove_receipt(witness, output_dir, elf, prove).await,
        Command::Checkpoint {
            witness,
            output_dir,
            elf,
            prove,
        } => prove_checkpoint(witness, output_dir, elf, prove).await,
    }
}

async fn program_info(elf_path: Option<PathBuf>, output: Option<PathBuf>) -> Result<()> {
    let elf = load_elf(elf_path.as_ref())?;
    let client = ProverClient::from_env().await;
    let proving_key = client.setup(elf.clone()).await?;
    let report = serde_json::json!({
        "schema": "postfiat-pftl-uniswap-program-info-v1",
        "elf_sha256": hex::encode(Sha256::digest(&*elf)),
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

async fn prove_receipt(
    witness_path: PathBuf,
    output_dir: PathBuf,
    elf_path: Option<PathBuf>,
    prove: bool,
) -> Result<()> {
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
        load_elf(elf_path.as_ref())?,
        prove,
        PftlUniswapProofInputV1::Receipt(Box::new(witness)),
        expected,
        "receipt",
    )
    .await
}

async fn prove_checkpoint(
    witness_path: PathBuf,
    output_dir: PathBuf,
    elf_path: Option<PathBuf>,
    prove: bool,
) -> Result<()> {
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
        load_elf(elf_path.as_ref())?,
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
    elf: Elf,
    prove: bool,
    input: PftlUniswapProofInputV1,
    expected_public_values: Vec<u8>,
    proof_kind: &str,
) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let mut cbor_value =
        serde_cbor::value::to_value(&input).context("encode witness as CBOR value")?;
    normalize_cbor_byte_strings(&mut cbor_value);
    let encoded_input =
        serde_cbor::to_vec(&cbor_value).context("encode witness as versioned CBOR")?;
    let decoded_input: PftlUniswapProofInputV1 =
        serde_cbor::from_slice(&encoded_input).context("decode host CBOR witness round trip")?;
    let roundtrip_public_values = match decoded_input {
        PftlUniswapProofInputV1::Receipt(witness) => {
            verify_pftl_uniswap_receipt_witness_v1(&witness)
                .map_err(|error| anyhow::anyhow!("host CBOR receipt round trip failed: {error}"))?
                .abi_encode()
        }
        PftlUniswapProofInputV1::Checkpoint(witness) => {
            verify_pftl_uniswap_checkpoint_witness_v1(&witness)
                .map_err(|error| {
                    anyhow::anyhow!("host CBOR checkpoint round trip failed: {error}")
                })?
                .abi_encode()
        }
    };
    anyhow::ensure!(
        roundtrip_public_values == expected_public_values,
        "host CBOR round trip changed canonical public values"
    );
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(encoded_input);
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (public_values, report) = client.execute(elf.clone(), stdin.clone()).await?;
    let executed = public_values.to_vec();
    if executed != expected_public_values {
        fs::create_dir_all(&output_dir)?;
        fs::write(output_dir.join("public-values.executed.bin"), &executed)?;
        fs::write(
            output_dir.join("public-values.expected.bin"),
            &expected_public_values,
        )?;
        fs::write(
            output_dir.join("mismatch-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "executed_public_values_bytes": executed.len(),
                "expected_public_values_bytes": expected_public_values.len(),
                "instruction_count": report.total_instruction_count(),
                "elapsed_ms": started.elapsed().as_millis(),
            }))?,
        )?;
    }
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
        let proving_key = client.setup(elf).await?;
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

fn load_elf(path: Option<&PathBuf>) -> Result<Elf> {
    match path {
        Some(path) => {
            Ok(Elf::from(fs::read(path).with_context(|| {
                format!("read guest ELF {}", path.display())
            })?))
        }
        None => Ok(RECEIPT_ELF),
    }
}

/// Hash48 protocol newtypes historically encoded as CBOR byte strings while
/// their deployed guest expected a sequence. Sequences are accepted by both
/// the deployed decoder and the corrected decoder, so normalize the transport
/// without changing the proof statement or deployed program key.
fn normalize_cbor_byte_strings(value: &mut serde_cbor::Value) {
    match value {
        serde_cbor::Value::Bytes(bytes) => {
            *value = serde_cbor::Value::Array(
                bytes
                    .iter()
                    .copied()
                    .map(|byte| serde_cbor::Value::Integer(byte.into()))
                    .collect(),
            );
        }
        serde_cbor::Value::Array(values) => {
            for value in values {
                normalize_cbor_byte_strings(value);
            }
        }
        serde_cbor::Value::Map(entries) => {
            for value in entries.values_mut() {
                normalize_cbor_byte_strings(value);
            }
        }
        serde_cbor::Value::Tag(_, value) => normalize_cbor_byte_strings(value),
        _ => {}
    }
}
