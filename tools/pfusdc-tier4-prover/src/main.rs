use std::{fs, path::PathBuf, time::Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pfusdc_ingress_program::{verify_ingress_witness_v2, PfUsdcIngressProofWitnessV2};
use postfiat_pfusdc_proofs::{verify_checkpoint_witness_v1, verify_egress_witness_v1};
use postfiat_types::{
    PfUsdcCheckpointProofWitnessV1, PfUsdcEgressProgramInputV1, PfUsdcEgressProofWitnessV1,
    PfUsdcIngressPublicValuesV3,
};
use sha2::Sha256;
use sha3::{Digest, Sha3_384};
use sp1_sdk::{Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Stdin};

const EGRESS_ELF: Elf = Elf::Static(include_bytes!(
    "../../../programs/pfusdc-egress/elf/pfusdc-egress-program"
));
const INGRESS_ELF: Elf = Elf::Static(include_bytes!(
    "../../../programs/pfusdc-ingress/elf/pfusdc-ingress-program"
));

mod egress_audit;
mod ingress_capture;
mod manifest;

#[derive(Debug, Parser)]
#[command(name = "pfusdc-tier4-prover")]
#[command(about = "Proof builder for the proof-native pfUSDC Tier-4 route")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Derive the immutable ELF hashes and SP1 program verifying keys.
    ProgramInfo {
        /// Optional JSON output path. The same document is always printed.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Build and cross-check the deterministic Tier-4 route/deployment manifest.
    DeploymentManifest {
        /// JSON input containing the frozen chain, network, and artifact values.
        #[arg(long)]
        input: PathBuf,
        /// Canonical pretty-JSON output path.
        #[arg(long)]
        output: PathBuf,
    },
    /// Capture and natively verify one finalized Ethereum/Arbitrum ingress witness.
    IngressCapture(ingress_capture::IngressCaptureArgs),
    /// Capture the governed Ethereum/Arbitrum checkpoint from which ingress must advance.
    FinalityBootstrap(ingress_capture::FinalityBootstrapArgs),
    /// Run the bounded security-field mutation matrix against a captured witness.
    IngressAudit(ingress_capture::IngressAuditArgs),
    /// Run the bounded consensus/exit mutation matrix against a captured egress witness.
    EgressAudit(egress_audit::EgressAuditArgs),
    /// Execute or Groth16-prove a canonical Ethereum/Arbitrum ingress witness.
    Ingress {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a canonical PFTL egress witness.
    Egress {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a bounded PFTL checkpoint-only segment.
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
    let args = Args::parse();
    match args.command {
        Command::ProgramInfo { output } => program_info(output).await,
        Command::DeploymentManifest { input, output } => manifest::run(input, output),
        Command::IngressCapture(capture) => ingress_capture::capture(capture).await,
        Command::FinalityBootstrap(capture) => {
            ingress_capture::capture_finality_bootstrap(capture).await
        }
        Command::IngressAudit(audit) => ingress_capture::audit(audit),
        Command::EgressAudit(audit) => egress_audit::audit(audit),
        Command::Ingress {
            witness,
            output_dir,
            prove,
        } => prove_ingress(witness, output_dir, prove).await,
        Command::Egress {
            witness,
            output_dir,
            prove,
        } => prove_egress(witness, output_dir, prove).await,
        Command::Checkpoint {
            witness,
            output_dir,
            prove,
        } => prove_checkpoint(witness, output_dir, prove).await,
    }
}

async fn program_info(output: Option<PathBuf>) -> Result<()> {
    let client = ProverClient::from_env().await;
    let ingress = client.setup(INGRESS_ELF).await?;
    let egress = client.setup(EGRESS_ELF).await?;
    let document = serde_json::json!({
        "schema": "postfiat.pfusdc.tier4_program_info.v1",
        "sp1_sdk_version": "6.3.1",
        "ingress": {
            "elf_sha256": hex::encode(Sha256::digest(&*INGRESS_ELF)),
            "program_vkey": ingress.verifying_key().bytes32(),
        },
        "egress": {
            "elf_sha256": hex::encode(Sha256::digest(&*EGRESS_ELF)),
            "program_vkey": egress.verifying_key().bytes32(),
        },
    });
    let bytes = serde_json::to_vec_pretty(&document)?;
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &bytes)?;
    }
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}

async fn prove_ingress(witness_path: PathBuf, output_dir: PathBuf, prove: bool) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read ingress witness {}", witness_path.display()))?;
    let witness: PfUsdcIngressProofWitnessV2 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode ingress witness {}", witness_path.display()))?;
    let expected = verify_ingress_witness_v2(&witness)
        .map_err(|error| anyhow::anyhow!("native ingress witness verification failed: {error}"))?;
    let expected_public_values = expected
        .canonical_bytes_without_commitment()
        .map_err(|error| anyhow::anyhow!("encode expected public values: {error}"))?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(serde_cbor::to_vec(&witness).context("encode ingress witness as CBOR")?);
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(INGRESS_ELF, stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    if executed != expected_public_values {
        let actual = PfUsdcIngressPublicValuesV3::from_canonical_bytes(&executed).map_err(
            |error| {
                anyhow::anyhow!(
                    "decode SP1 ingress output: {error}; bytes={}, hex={}",
                    executed.len(),
                    hex::encode(&executed)
                )
            },
        )?;
        let expected_json = serde_json::to_value(&expected)?;
        let actual_json = serde_json::to_value(&actual)?;
        let differences = expected_json
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(key, expected_value)| {
                let actual_value = actual_json.get(key);
                (actual_value != Some(expected_value)).then(|| {
                    serde_json::json!({
                        "field": key,
                        "expected": expected_value,
                        "actual": actual_value,
                    })
                })
            })
            .collect::<Vec<_>>();
        anyhow::bail!(
            "SP1 ingress output differs from native canonical public values: {}",
            serde_json::to_string(&differences)?
        );
    }
    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    fs::write(
        output_dir.join("public-values.sha3-384"),
        hex::encode(Sha3_384::digest(&executed)),
    )?;
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat.pfusdc.ingress_execute_report.v1",
            "witness": witness_path,
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    println!(
        "ingress witness executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    if prove {
        let setup_started = Instant::now();
        let pk = client.setup(INGRESS_ELF).await?;
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
                "schema": "postfiat.pfusdc.ingress_proof_report.v1",
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
    stdin.write_vec(
        serde_cbor::to_vec(&PfUsdcEgressProgramInputV1::Withdrawal(witness.clone()))
            .context("encode egress witness as CBOR")?,
    );
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(EGRESS_ELF, stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    if executed != expected_public_values {
        fs::create_dir_all(&output_dir)?;
        fs::write(
            output_dir.join("guest-public-values.mismatch.bin"),
            &executed,
        )?;
        fs::write(
            output_dir.join("native-public-values.mismatch.bin"),
            &expected_public_values,
        )?;
        let first_difference = executed
            .iter()
            .zip(&expected_public_values)
            .position(|(guest, native)| guest != native)
            .unwrap_or(executed.len().min(expected_public_values.len()));
        anyhow::bail!(
            "SP1 egress output differs from native canonical public values at byte {first_difference} (guest {} bytes, native {} bytes, cycles {})",
            executed.len(),
            expected_public_values.len(),
            report.total_instruction_count()
        );
    }
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

async fn prove_checkpoint(witness_path: PathBuf, output_dir: PathBuf, prove: bool) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read checkpoint witness {}", witness_path.display()))?;
    let witness: PfUsdcCheckpointProofWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode checkpoint witness {}", witness_path.display()))?;
    let expected = verify_checkpoint_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native checkpoint verification failed: {error}"))?;
    let expected_public_values = expected
        .canonical_bytes_without_commitment()
        .map_err(|error| anyhow::anyhow!("encode checkpoint public values: {error}"))?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(
        serde_cbor::to_vec(&PfUsdcEgressProgramInputV1::Checkpoint(witness))
            .context("encode checkpoint witness as CBOR")?,
    );
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(EGRESS_ELF, stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 checkpoint output differs from native canonical public values"
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
            "schema": "postfiat.pfusdc.checkpoint_execute_report.v1",
            "witness": witness_path,
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    if prove {
        let pk = client.setup(EGRESS_ELF).await?;
        let proof = client.prove(&pk, stdin).groth16().await?;
        client.verify(&proof, pk.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified checkpoint proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
    }
    println!(
        "checkpoint witness executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    Ok(())
}
