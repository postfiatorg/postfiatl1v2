use super::*;
use bincode::Options;
use sha2::{Digest, Sha256};
use sp1_sdk::{
    Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Proof,
    SP1ProofWithPublicValues, SP1Stdin,
};
use std::time::Instant;

fn runtime() -> Result<tokio::runtime::Runtime> {
    for name in [
        "SP1_DUMP",
        "SP1_RECORD_WRITE_DIR",
        "SP1_RECORD_MAX_ARITY_INPUT",
        "SP1_RECORD_SHRINK_INPUT",
        "TRACE_FILE",
        "DUMP_ELF_OUTPUT",
        "WITHOUT_VK_VERIFICATION",
        "SP1_CIRCUIT_MODE",
        "SP1_GROTH16_CIRCUIT_PATH",
        "SP1_PLONK_CIRCUIT_PATH",
    ] {
        ensure!(
            std::env::var_os(name).is_none(),
            "unsupported SP1 override: {name}"
        );
    }
    let _ = tracing_subscriber::fmt()
        .with_env_filter("sp1_sdk=info,sp1_prover=info")
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}
fn input(bytes: Vec<u8>) -> SP1Stdin {
    let mut s = SP1Stdin::new();
    s.write_vec(bytes);
    s
}
fn elf(path: &PathBuf) -> Result<(Elf, String)> {
    let b = read(path, 64 * 1024 * 1024)?;
    let hash = hex::encode(Sha256::digest(&b));
    Ok((Elf::from(b), hash))
}
pub fn identity(path: PathBuf) -> Result<()> {
    let (elf, hash) = elf(&path)?;
    let vkey = runtime()?.block_on(async {
        let c = ProverClient::builder().light().build().await;
        Ok::<_, anyhow::Error>(c.setup(elf).await?.verifying_key().bytes32())
    })?;
    report(serde_json::json!({"elf_sha256":hash,"program_vkey":vkey,"sp1_version":"6.3.1"}))
}
pub fn execute(witness: PathBuf, path: PathBuf, output: PathBuf) -> Result<()> {
    let (bytes, expected) = native(&witness)?;
    let (elf, hash) = elf(&path)?;
    let start = Instant::now();
    let (values, r) = runtime()?.block_on(async {
        ProverClient::builder()
            .light()
            .build()
            .await
            .execute(elf, input(bytes))
            .await
    })?;
    ensure!(values.to_vec() == expected, "guest/native bytes differ");
    write(&output, &expected)?;
    report(
        serde_json::json!({"guest_verified":true,"elf_sha256":hash,"instruction_count":r.total_instruction_count(),"elapsed_ms":start.elapsed().as_millis()}),
    )
}
pub fn prove(witness: PathBuf, path: PathBuf, output: PathBuf) -> Result<()> {
    ensure!(!cfg!(debug_assertions), "proving requires release build");
    ensure!(!output.exists(), "output exists");
    let (bytes, expected) = native(&witness)?;
    let (elf, hash) = elf(&path)?;
    let start = Instant::now();
    let (proof, vkey) = runtime()?.block_on(async {
        #[cfg(feature = "cuda")]
        let client = ProverClient::builder().cuda().build().await;
        #[cfg(not(feature = "cuda"))]
        let client = ProverClient::builder().cpu().build().await;
        let key = client.setup(elf).await?;
        let proof = client.prove(&key, input(bytes)).groth16().await?;
        client.verify(&proof, key.verifying_key(), None)?;
        Ok::<_, anyhow::Error>((proof, key.verifying_key().bytes32()))
    })?;
    ensure!(
        proof.public_values.to_vec() == expected,
        "proof/native bytes differ"
    );
    fs::create_dir(&output)?;
    write(&output.join("proof.bin"), &bincode::serialize(&proof)?)?;
    write(&output.join("proof-calldata.bin"), &proof.bytes())?;
    write(&output.join("public-values.bin"), &expected)?;
    let r = serde_json::json!({"proof_verified":true,"elf_sha256":hash,"program_vkey":vkey,"backend":if cfg!(feature="cuda"){"cuda"}else{"cpu"},"elapsed_ms":start.elapsed().as_millis(),"proof_bytes":proof.bytes().len(),"public_values_bytes":expected.len()});
    write(
        &output.join("proof-report.json"),
        &serde_json::to_vec_pretty(&r)?,
    )?;
    report(r)
}
pub fn verify(witness: PathBuf, path: PathBuf, proof: PathBuf) -> Result<()> {
    let (_, expected) = native(&witness)?;
    let (elf, hash) = elf(&path)?;
    let proof: SP1ProofWithPublicValues = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .with_limit(32 * 1024 * 1024)
        .deserialize(&read(&proof, 32 * 1024 * 1024)?)?;
    ensure!(
        matches!(&proof.proof, SP1Proof::Groth16(_)),
        "require Groth16"
    );
    ensure!(
        proof.public_values.to_vec() == expected,
        "proof/native bytes differ"
    );
    let key = runtime()?.block_on(async {
        let client = ProverClient::builder().light().build().await;
        let key = client.setup(elf).await?;
        client.verify(&proof, key.verifying_key(), None)?;
        Ok::<_, anyhow::Error>(key.verifying_key().bytes32())
    })?;
    report(serde_json::json!({"independently_verified":true,"elf_sha256":hash,"program_vkey":key}))
}
