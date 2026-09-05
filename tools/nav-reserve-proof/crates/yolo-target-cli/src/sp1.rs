//! Explicit local CPU proving; no environment-selected network prover.

use super::*;
use bincode::Options;
use sp1_sdk::{
    Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Proof,
    SP1ProofWithPublicValues, SP1Stdin,
};
use std::time::Instant;

const MAX_ELF_BYTES: usize = 64 * 1024 * 1024;
const MAX_PROOF_BYTES: usize = 32 * 1024 * 1024;

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}

fn pinned_elf(path: &PathBuf, pins: &TargetAcceptanceV1) -> Result<Elf> {
    let bytes = read_bounded(path, MAX_ELF_BYTES, "SP1 ELF")?;
    anyhow::ensure!(
        hex::encode(Sha256::digest(&bytes)) == pins.program_sha256,
        "ELF differs from the independently approved program SHA-256"
    );
    Ok(Elf::from(bytes))
}

fn input(bytes: Vec<u8>) -> SP1Stdin {
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(bytes);
    stdin
}

pub(super) fn execute(
    witness: PathBuf,
    acceptance: PathBuf,
    elf: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let encoded = read_bounded(&witness, MAX_TARGET_WITNESS_BYTES, "private target witness")?;
    let (expected, pins) = accepted_native(&encoded, &acceptance)?;
    let elf = pinned_elf(&elf, &pins)?;
    let witness_bytes = encoded.len();
    let start = Instant::now();
    let (actual, execution) = runtime()?.block_on(async {
        let client = ProverClient::builder().cpu().build().await;
        client.execute(elf, input(encoded)).await
    })?;
    let expected = expected.encode().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        actual.to_vec() == expected,
        "SP1 public values differ from native execution"
    );
    write_new(&output, &expected)?;
    report(&serde_json::json!({
        "schema": "postfiat.yolo.guest_execution.v1", "witnessBytes": witness_bytes,
        "publicValuesBytes": expected.len(), "instructionCount": execution.total_instruction_count(),
        "touchedMemoryAddresses": (execution.touched_memory_addresses > 0).then_some(execution.touched_memory_addresses),
        "syscallCount": (execution.total_syscall_count() > 0).then_some(execution.total_syscall_count()),
        "gas": execution.gas(),
        "elapsedMilliseconds": start.elapsed().as_millis(),
    }))
}

pub(super) fn prove(
    witness: PathBuf,
    acceptance: PathBuf,
    elf: PathBuf,
    output: PathBuf,
) -> Result<()> {
    anyhow::ensure!(
        !cfg!(debug_assertions),
        "Groth16 proving requires a release build"
    );
    anyhow::ensure!(!output.exists(), "proof output directory already exists");
    let encoded = read_bounded(&witness, MAX_TARGET_WITNESS_BYTES, "private target witness")?;
    let (expected, pins) = accepted_native(&encoded, &acceptance)?;
    let elf = pinned_elf(&elf, &pins)?;
    let start = Instant::now();
    let (proof, vkey, verify_ms) = runtime()?.block_on(async {
        let client = ProverClient::builder().cpu().build().await;
        let key = client.setup(elf).await?;
        let proof = client.prove(&key, input(encoded)).groth16().await?;
        let verify_start = Instant::now();
        client.verify(&proof, key.verifying_key(), None)?;
        Ok::<_, anyhow::Error>((
            proof,
            key.verifying_key().bytes32(),
            verify_start.elapsed().as_millis(),
        ))
    })?;
    let expected = expected.encode().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        proof.public_values.to_vec() == expected,
        "proved public values differ from native execution"
    );
    fs::create_dir(&output)?;
    write_new(&output.join("proof.bin"), &bincode::serialize(&proof)?)?;
    write_new(&output.join("proof-calldata.bin"), &proof.bytes())?;
    write_new(&output.join("public-values.bin"), &expected)?;
    let value = serde_json::json!({
        "schema": "postfiat.yolo.groth16_proof.v1", "programVkey": vkey,
        "programSha256": pins.program_sha256, "collectionManifestSha256": pins.collection_manifest_sha256,
        "elapsedMilliseconds": start.elapsed().as_millis(), "verificationMilliseconds": verify_ms,
        "proofBytes": proof.bytes().len(), "publicValuesBytes": expected.len(),
    });
    write_new(
        &output.join("proof-report.json"),
        &serde_json::to_vec_pretty(&value)?,
    )?;
    report(&value)
}

pub(super) fn verify(proof_path: PathBuf, acceptance_path: PathBuf, elf: PathBuf) -> Result<()> {
    let pins = acceptance(&acceptance_path)?;
    let encoded = read_bounded(&proof_path, MAX_PROOF_BYTES, "serialized SP1 proof")?;
    let proof: SP1ProofWithPublicValues = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .with_limit(MAX_PROOF_BYTES as u64)
        .deserialize(&encoded)?;
    anyhow::ensure!(
        matches!(&proof.proof, SP1Proof::Groth16(_)),
        "target receipt requires a Groth16 proof"
    );
    let public = YoloTargetPublicValuesV1::decode(&proof.public_values.to_vec())
        .map_err(anyhow::Error::msg)?;
    pins.verify(&public).map_err(anyhow::Error::msg)?;
    let elf = pinned_elf(&elf, &pins)?;
    let start = Instant::now();
    let vkey = runtime()?.block_on(async {
        let client = ProverClient::builder().cpu().build().await;
        let key = client.setup(elf).await?;
        client.verify(&proof, key.verifying_key(), None)?;
        Ok::<_, anyhow::Error>(key.verifying_key().bytes32())
    })?;
    report(
        &serde_json::json!({"schema": "postfiat.yolo.proof_verification.v1", "valid": true,
        "programVkey": vkey, "publicValues": public, "elapsedMilliseconds": start.elapsed().as_millis()}),
    )
}

pub(super) fn identity(elf: PathBuf) -> Result<()> {
    let bytes = read_bounded(&elf, MAX_ELF_BYTES, "SP1 ELF")?;
    let hash = hex::encode(Sha256::digest(&bytes));
    let key = runtime()?.block_on(async {
        let client = ProverClient::builder().cpu().build().await;
        let key = client.setup(Elf::from(bytes)).await?;
        Ok::<_, anyhow::Error>(key.verifying_key().bytes32())
    })?;
    report(
        &serde_json::json!({"schema": "postfiat.yolo.program_identity.v1", "elfSha256": hash, "programVkey": key, "sp1Version": "6.3.1"}),
    )
}
