use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    path::PathBuf,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use postfiat_nav_reserve_protocol::nav_reserve_subscription_composite_source_root_v1;
use postfiat_types::{
    AssetTransactionOperation, NavProfileRegisterOperation, NavProofProfile,
    NavReservePublicValuesV1, NavReserveSubmitOperation, SignedAssetTransaction,
    DEFAULT_MAX_NAV_SP1_PROOF_BYTES,
};
use reserve_proof_types::{
    execute_reserve_proof, opaque_commitment, ReserveProofContextV1, ReserveProofWitnessV1,
    SourceManifestV1, SourceObservationV1, MAX_EVIDENCE_BYTES, MAX_WITNESS_BYTES,
    WITNESS_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};

mod evm_adapter;
mod hyperliquid_adapter;
mod monero_adapter;
mod near_adapter;
mod source_checkpoint;

use evm_adapter::AdapterCommand;
use source_checkpoint::SourceCheckpointCommand;

#[cfg(feature = "sp1")]
use bincode::Options as _;
#[cfg(feature = "sp1")]
use sha2::{Digest as _, Sha256};
#[cfg(all(feature = "sp1", not(debug_assertions)))]
use sp1_sdk::ProveRequest;
#[cfg(feature = "sp1")]
use sp1_sdk::{
    Elf, HashableKey, Prover, ProverClient, ProvingKey, SP1ProofWithPublicValues, SP1Stdin,
};

#[derive(Debug, Parser)]
#[command(name = "postfiat-reserve-proof")]
#[command(about = "Provider-neutral NAV reserve manifest, witness, proof, and packet tool")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Adapter {
        #[command(subcommand)]
        command: Box<AdapterCommand>,
    },
    /// Build and validate provider-neutral BFT certificates for external
    /// source checkpoints. Signing remains isolated to each validator.
    SourceCheckpoint {
        #[command(subcommand)]
        command: SourceCheckpointCommand,
    },
    Manifest {
        #[command(subcommand)]
        command: ManifestCommand,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    Commitment {
        #[command(subcommand)]
        command: CommitmentCommand,
    },
    /// Assemble canonically ordered adapter observations into a reviewed JSON
    /// witness. Each input is `<source_id>.json` in `input_dir`.
    Observe {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        input_dir: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Witness {
        #[command(subcommand)]
        command: WitnessCommand,
    },
    Execute {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        elf: Option<PathBuf>,
    },
    Prove {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
    },
    Verify {
        #[arg(long)]
        public_values: Option<PathBuf>,
        #[arg(long)]
        proof: Option<PathBuf>,
        #[arg(long)]
        elf: Option<PathBuf>,
    },
    ProgramInfo {
        #[arg(long)]
        elf: PathBuf,
    },
    Packet {
        #[command(subcommand)]
        command: PacketCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ManifestCommand {
    Validate { manifest: PathBuf },
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    /// Derive the exact consensus profile ID and immutable profile from a
    /// reviewed nav_profile_register operation.
    Derive {
        #[arg(long)]
        registration: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum CommitmentCommand {
    /// Commit a bounded public artifact under a labeled provider-neutral
    /// opaque commitment domain.
    Derive {
        #[arg(long)]
        label: String,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum WitnessCommand {
    Build {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum PacketCommand {
    Build {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        public_values: PathBuf,
        #[arg(long)]
        proof_calldata: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Submit {
        /// Locally signed PFTL asset transaction containing exactly one
        /// nav_reserve_submit operation.
        #[arg(long)]
        signed_transaction: PathBuf,
        /// PFTL newline-JSON RPC endpoint as HOST:PORT. Repeat to try the
        /// validator fleet; only typed wrong-proposer errors are retried.
        #[arg(long, required = true)]
        rpc_address: Vec<String>,
        /// Write the accepted finality response as durable evidence.
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 45_000)]
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PacketTemplateV1 {
    schema: String,
    issuer: String,
    submitter: String,
    nav_per_unit: u64,
    circulating_supply: u64,
    source_root: String,
    attestor_root: String,
    reserve_packet_hash: String,
    /// Optional consensus-accounted NAV subscription reserve. When present,
    /// `source_root` must be the exact composite root over the proof public
    /// values and this overlay, and the packet carries base proof assets plus
    /// the overlay value. This is the A666/pfUSDC primary-market shape.
    #[serde(default)]
    subscription_overlay_source_root: Option<String>,
    #[serde(default)]
    subscription_overlay_value: u64,
}

#[derive(Debug, Clone, Serialize)]
struct DerivedProfileV1 {
    schema: &'static str,
    operation: NavProfileRegisterOperation,
    profile: NavProofProfile,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Adapter { command } => evm_adapter::run(*command),
        Command::SourceCheckpoint { command } => source_checkpoint::run(command),
        Command::Manifest {
            command: ManifestCommand::Validate { manifest },
        } => manifest_validate(manifest),
        Command::Profile {
            command:
                ProfileCommand::Derive {
                    registration,
                    output,
                },
        } => profile_derive(registration, output),
        Command::Commitment {
            command:
                CommitmentCommand::Derive {
                    label,
                    input,
                    output,
                },
        } => commitment_derive(label, input, output),
        Command::Observe {
            manifest,
            context,
            input_dir,
            output,
        } => observe(manifest, context, input_dir, output),
        Command::Witness {
            command: WitnessCommand::Build { input, output },
        } => witness_build(input, output),
        Command::Execute {
            witness,
            output,
            elf,
        } => execute(witness, output, elf),
        Command::Prove {
            witness,
            elf,
            output_dir,
        } => prove(witness, elf, output_dir),
        Command::Verify {
            public_values,
            proof,
            elf,
        } => verify(public_values, proof, elf),
        Command::ProgramInfo { elf } => program_info(elf),
        Command::Packet {
            command:
                PacketCommand::Build {
                    template,
                    public_values,
                    proof_calldata,
                    output,
                },
        } => packet_build(template, public_values, proof_calldata, output),
        Command::Packet {
            command:
                PacketCommand::Submit {
                    signed_transaction,
                    rpc_address,
                    output,
                    timeout_ms,
                },
        } => packet_submit(signed_transaction, rpc_address, output, timeout_ms),
    }
}

// Aave state proofs contain many MPT nodes and are carried in both evidence
// dimensions because one proof verifies quantity and oracle valuation. Keep
// this below the 8 MiB complete-witness bound while permitting that canonical
// JSON representation.
const MAX_OBSERVATION_JSON_BYTES: u64 = 4 * 1024 * 1024;
#[cfg(feature = "sp1")]
const MAX_GUEST_ELF_BYTES: usize = 64 * 1024 * 1024;
#[cfg(feature = "sp1")]
const MAX_SERIALIZED_SP1_PROOF_BYTES: usize = 16 * 1024 * 1024;
const MAX_PUBLIC_VALUES_INPUT_BYTES: usize = 64 * 1024;

fn profile_derive(registration: PathBuf, output: PathBuf) -> Result<()> {
    let operation: NavProfileRegisterOperation = read_json(&registration)?;
    let profile = operation.to_profile().map_err(anyhow::Error::msg)?;
    write_new(
        &output,
        &serde_json::to_vec_pretty(&DerivedProfileV1 {
            schema: "postfiat.reserve_derived_profile.v1",
            operation,
            profile,
        })?,
    )
}

fn commitment_derive(label: String, input: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let bytes = read_bounded(&input, MAX_EVIDENCE_BYTES, "opaque commitment input")?;
    let commitment = opaque_commitment(&label, &bytes).map_err(anyhow::Error::msg)?;
    if let Some(output) = output {
        write_new(&output, commitment.as_bytes())?;
    }
    println!("{commitment}");
    Ok(())
}

fn observe(
    manifest_path: PathBuf,
    context_path: PathBuf,
    input_dir: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let manifest: SourceManifestV1 = read_json(&manifest_path)?;
    manifest.validate().map_err(anyhow::Error::msg)?;
    let context: ReserveProofContextV1 = read_json(&context_path)?;
    anyhow::ensure!(
        context.source_manifest_hash == manifest.hash().map_err(anyhow::Error::msg)?,
        "observation context source_manifest_hash does not match manifest"
    );
    let mut observations = Vec::with_capacity(manifest.sources.len());
    for source in &manifest.sources {
        let path = input_dir.join(format!("{}.json", source.source_id));
        let metadata = fs::metadata(&path)
            .with_context(|| format!("stat adapter observation {}", path.display()))?;
        anyhow::ensure!(
            metadata.is_file(),
            "adapter observation is not a regular file: {}",
            path.display()
        );
        anyhow::ensure!(
            metadata.len() <= MAX_OBSERVATION_JSON_BYTES,
            "adapter observation exceeds {MAX_OBSERVATION_JSON_BYTES} bytes: {}",
            path.display()
        );
        let observation: SourceObservationV1 = read_json(&path)?;
        anyhow::ensure!(
            observation.source_id == source.source_id,
            "adapter observation source_id mismatch for {}",
            source.source_id
        );
        observations.push(observation);
    }
    let witness = ReserveProofWitnessV1 {
        schema: WITNESS_SCHEMA_V1.to_string(),
        context,
        manifest,
        observations,
    };
    let values = execute_reserve_proof(&witness).map_err(anyhow::Error::msg)?;
    write_new(&output, &serde_json::to_vec_pretty(&witness)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_observe.v1",
            "witness": output,
            "source_count": witness.observations.len(),
            "source_manifest_hash": witness.context.source_manifest_hash,
            "source_observation_root": values.source_observation_root,
            "verified_net_assets": values.verified_net_assets,
        }))?
    );
    Ok(())
}

fn manifest_validate(path: PathBuf) -> Result<()> {
    let manifest: SourceManifestV1 = read_json(&path)?;
    manifest.validate().map_err(anyhow::Error::msg)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_manifest_validation.v1",
            "valid": true,
            "source_count": manifest.sources.len(),
            "source_manifest_hash": manifest.hash().map_err(anyhow::Error::msg)?,
        }))?
    );
    Ok(())
}

fn witness_build(input: PathBuf, output: PathBuf) -> Result<()> {
    let witness: ReserveProofWitnessV1 = read_json(&input)?;
    let expected = execute_reserve_proof(&witness).map_err(anyhow::Error::msg)?;
    let mut value = serde_cbor::value::to_value(&witness)?;
    normalize_cbor_bytes(&mut value);
    let encoded = serde_cbor::to_vec(&value)?;
    anyhow::ensure!(
        encoded.len() <= MAX_WITNESS_BYTES,
        "canonical witness exceeds {MAX_WITNESS_BYTES} bytes"
    );
    let decoded: ReserveProofWitnessV1 = serde_cbor::from_slice(&encoded)?;
    anyhow::ensure!(
        execute_reserve_proof(&decoded).map_err(anyhow::Error::msg)? == expected,
        "CBOR round trip changed reserve public values"
    );
    write_new(&output, &encoded)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_witness_build.v1",
            "witness": output,
            "witness_bytes": encoded.len(),
            "source_count": witness.observations.len(),
            "source_manifest_hash": witness.context.source_manifest_hash,
        }))?
    );
    Ok(())
}

fn execute(witness_path: PathBuf, output: PathBuf, elf: Option<PathBuf>) -> Result<()> {
    let encoded = read_bounded(&witness_path, MAX_WITNESS_BYTES, "reserve witness")?;
    let witness: ReserveProofWitnessV1 = serde_cbor::from_slice(&encoded)?;
    let expected = execute_reserve_proof(&witness)
        .map_err(anyhow::Error::msg)?
        .encode()
        .map_err(anyhow::Error::msg)?;
    if let Some(ref elf_path) = elf {
        execute_sp1(&encoded, &expected, elf_path)?;
    }
    write_new(&output, &expected)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_execute.v1",
            "public_values": output,
            "public_values_bytes": expected.len(),
            "sp1_executed": elf.is_some(),
        }))?
    );
    Ok(())
}

#[cfg(feature = "sp1")]
fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}

#[cfg(feature = "sp1")]
fn execute_sp1(witness: &[u8], expected: &[u8], elf_path: &PathBuf) -> Result<()> {
    let elf = Elf::from(read_bounded(
        elf_path,
        MAX_GUEST_ELF_BYTES,
        "SP1 guest ELF",
    )?);
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(witness.to_vec());
    let (actual, _) = runtime()?.block_on(async {
        let client = ProverClient::from_env().await;
        client.execute(elf, stdin).await
    })?;
    anyhow::ensure!(
        actual.to_vec() == expected,
        "SP1 output differs from native execution"
    );
    Ok(())
}

#[cfg(not(feature = "sp1"))]
fn execute_sp1(_: &[u8], _: &[u8], _: &PathBuf) -> Result<()> {
    bail!("SP1 execution requires --features sp1")
}

#[cfg(all(feature = "sp1", not(debug_assertions)))]
fn prove(witness_path: PathBuf, elf_path: PathBuf, output_dir: PathBuf) -> Result<()> {
    let encoded = read_bounded(&witness_path, MAX_WITNESS_BYTES, "reserve witness")?;
    let witness: ReserveProofWitnessV1 = serde_cbor::from_slice(&encoded)?;
    let expected = execute_reserve_proof(&witness)
        .map_err(anyhow::Error::msg)?
        .encode()
        .map_err(anyhow::Error::msg)?;
    let elf = Elf::from(read_bounded(
        &elf_path,
        MAX_GUEST_ELF_BYTES,
        "SP1 guest ELF",
    )?);
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(encoded);
    let (proof, vkey) = runtime()?.block_on(async {
        let client = ProverClient::from_env().await;
        let key = client.setup(elf).await?;
        let proof = client.prove(&key, stdin).groth16().await?;
        client.verify(&proof, key.verifying_key(), None)?;
        Ok::<_, anyhow::Error>((proof, key.verifying_key().bytes32()))
    })?;
    anyhow::ensure!(
        proof.public_values.to_vec() == expected,
        "proof public values mismatch"
    );
    fs::create_dir_all(&output_dir)?;
    let proof_calldata = proof.bytes();
    write_new(&output_dir.join("proof.bin"), &bincode::serialize(&proof)?)?;
    write_new(&output_dir.join("proof-calldata.bin"), &proof_calldata)?;
    write_new(&output_dir.join("public-values.bin"), &expected)?;
    write_new(
        &output_dir.join("proof-report.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_groth16_proof.v1",
            "program_vkey": vkey,
            "proof_bytes": proof_calldata.len(),
            "public_values_bytes": expected.len(),
        }))?,
    )?;
    Ok(())
}

#[cfg(all(feature = "sp1", debug_assertions))]
fn prove(_: PathBuf, _: PathBuf, _: PathBuf) -> Result<()> {
    bail!("Groth16 proving requires a --release build")
}

#[cfg(not(feature = "sp1"))]
fn prove(_: PathBuf, _: PathBuf, _: PathBuf) -> Result<()> {
    bail!("SP1 proving requires --features sp1")
}

fn verify(
    public_values: Option<PathBuf>,
    proof: Option<PathBuf>,
    elf: Option<PathBuf>,
) -> Result<()> {
    match (public_values, proof, elf) {
        (Some(path), None, None) => {
            let decoded = NavReservePublicValuesV1::decode(&read_bounded(
                &path,
                MAX_PUBLIC_VALUES_INPUT_BYTES,
                "reserve public values",
            )?)
            .map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&decoded)?);
            Ok(())
        }
        (None, Some(proof), Some(elf)) => verify_sp1_proof(proof, elf),
        _ => bail!("use either --public-values, or both --proof and --elf"),
    }
}

#[cfg(feature = "sp1")]
fn verify_sp1_proof(proof_path: PathBuf, elf_path: PathBuf) -> Result<()> {
    let proof_bytes = read_bounded(
        &proof_path,
        MAX_SERIALIZED_SP1_PROOF_BYTES,
        "serialized SP1 proof",
    )?;
    let proof: SP1ProofWithPublicValues = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .with_limit(MAX_SERIALIZED_SP1_PROOF_BYTES as u64)
        .deserialize(&proof_bytes)?;
    let decoded = NavReservePublicValuesV1::decode(&proof.public_values.to_vec())
        .map_err(anyhow::Error::msg)?;
    let elf = Elf::from(read_bounded(
        &elf_path,
        MAX_GUEST_ELF_BYTES,
        "SP1 guest ELF",
    )?);
    let vkey = runtime()?.block_on(async {
        let client = ProverClient::from_env().await;
        let key = client.setup(elf).await?;
        client.verify(&proof, key.verifying_key(), None)?;
        Ok::<_, anyhow::Error>(key.verifying_key().bytes32())
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_proof_verification.v1",
            "valid": true,
            "program_vkey": vkey,
            "public_values": decoded,
        }))?
    );
    Ok(())
}

#[cfg(not(feature = "sp1"))]
fn verify_sp1_proof(_: PathBuf, _: PathBuf) -> Result<()> {
    bail!("SP1 proof verification requires --features sp1")
}

#[cfg(feature = "sp1")]
fn program_info(elf_path: PathBuf) -> Result<()> {
    let bytes = read_bounded(&elf_path, MAX_GUEST_ELF_BYTES, "SP1 guest ELF")?;
    let elf = Elf::from(bytes.clone());
    let vkey = runtime()?.block_on(async {
        let client = ProverClient::from_env().await;
        let key = client.setup(elf).await?;
        Ok::<_, anyhow::Error>(key.verifying_key().bytes32())
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_program_info.v1",
            "elf_sha256": hex::encode(Sha256::digest(bytes)),
            "program_vkey": vkey,
        }))?
    );
    Ok(())
}

#[cfg(not(feature = "sp1"))]
fn program_info(_: PathBuf) -> Result<()> {
    bail!("SP1 program info requires --features sp1")
}

fn packet_build(
    template_path: PathBuf,
    public_values_path: PathBuf,
    proof_path: PathBuf,
    output: PathBuf,
) -> Result<()> {
    let template: PacketTemplateV1 = read_json(&template_path)?;
    anyhow::ensure!(
        template.schema == "postfiat.reserve_packet_template.v1",
        "packet template schema mismatch"
    );
    let public_values_bytes = read_bounded(
        &public_values_path,
        MAX_PUBLIC_VALUES_INPUT_BYTES,
        "reserve public values",
    )?;
    let values =
        NavReservePublicValuesV1::decode(&public_values_bytes).map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        template.attestor_root == values.valuation_trust_root,
        "packet template attestor root does not match proven valuation trust root"
    );
    let operation = build_packet_operation(
        template,
        &values,
        read_bounded(
            &proof_path,
            DEFAULT_MAX_NAV_SP1_PROOF_BYTES as usize,
            "SP1 Groth16 calldata",
        )?,
        public_values_bytes,
    )?;
    operation.validate().map_err(anyhow::Error::msg)?;
    write_new(&output, &serde_json::to_vec_pretty(&operation)?)?;
    Ok(())
}

fn build_packet_operation(
    template: PacketTemplateV1,
    values: &NavReservePublicValuesV1,
    proof_bytes: Vec<u8>,
    public_values_bytes: Vec<u8>,
) -> Result<NavReserveSubmitOperation> {
    let (source_root, verified_net_assets) = match template.subscription_overlay_source_root {
        Some(overlay_root) => {
            anyhow::ensure!(
                overlay_root.len() == 96
                    && overlay_root
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                "subscription overlay source root must be 48-byte lowercase hex"
            );
            anyhow::ensure!(
                template.subscription_overlay_value != 0,
                "subscription overlay value must be nonzero when its source root is present"
            );
            let encoded = values.encode().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                encoded == public_values_bytes,
                "decoded public values do not round-trip to the supplied canonical bytes"
            );
            nav_reserve_subscription_composite_source_root_v1(
                values,
                &overlay_root,
                template.subscription_overlay_value,
            )
            .map_err(anyhow::Error::msg)?
        }
        None => {
            anyhow::ensure!(
                template.subscription_overlay_value == 0,
                "subscription overlay value requires a source root"
            );
            (
                values.source_observation_root.clone(),
                values.verified_net_assets,
            )
        }
    };
    anyhow::ensure!(
        template.source_root == source_root,
        "packet template source root does not match proven public values and subscription overlay"
    );
    Ok(NavReserveSubmitOperation {
        issuer: template.issuer,
        submitter: template.submitter,
        asset_id: values.nav_asset_id.clone(),
        epoch: values.observation_epoch,
        nav_per_unit: template.nav_per_unit,
        circulating_supply: template.circulating_supply,
        verified_net_assets,
        proof_profile: values.proof_profile_id.clone(),
        source_root,
        attestor_root: values.valuation_trust_root.clone(),
        reserve_packet_hash: template.reserve_packet_hash,
        reserve_accounts: Vec::new(),
        sp1_proof_bytes: proof_bytes,
        sp1_public_values: public_values_bytes,
    })
}

const MAX_PFTL_RPC_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

fn packet_submit(
    signed_transaction_path: PathBuf,
    rpc_addresses: Vec<String>,
    output: PathBuf,
    timeout_ms: u64,
) -> Result<()> {
    anyhow::ensure!(
        timeout_ms > 0 && timeout_ms <= 300_000,
        "timeout_ms must be in 1..=300000"
    );
    anyhow::ensure!(
        !output.exists(),
        "refusing to overwrite {}",
        output.display()
    );
    let signed: SignedAssetTransaction = read_json(&signed_transaction_path)?;
    signed.validate().map_err(anyhow::Error::msg)?;
    let AssetTransactionOperation::NavReserveSubmit(operation) = &signed.unsigned.operation else {
        bail!("signed transaction must contain nav_reserve_submit");
    };
    operation.validate().map_err(anyhow::Error::msg)?;
    let signed_json = serde_json::to_string(&signed)?;
    let request = serde_json::json!({
        "version": "postfiat-local-rpc-v1",
        "id": "reserve-proof-packet-submit",
        "method": "mempool_submit_signed_asset_transaction_finality",
        "params": {
            "signed_asset_transaction_json": signed_json,
        },
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let timeout = Duration::from_millis(timeout_ms);
    let mut failures = Vec::new();
    for address in rpc_addresses {
        let endpoint = resolve_rpc_address(&address)?;
        match rpc_roundtrip(endpoint, &request_bytes, timeout) {
            Ok(response) if response.get("ok") == Some(&serde_json::Value::Bool(true)) => {
                require_accepted_finality(&response)?;
                write_new(&output, &serde_json::to_vec_pretty(&response)?)?;
                println!("{}", serde_json::to_string_pretty(&response)?);
                return Ok(());
            }
            Ok(response) => {
                let code = response
                    .pointer("/error/code")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("rpc_error");
                let message = response
                    .pointer("/error/message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("PFTL RPC rejected packet submission");
                failures.push(format!("{address}: {code}: {message}"));
                if code != "rpc_finality_wrong_proposer" {
                    bail!("packet finality submission failed: {}", failures.join("; "));
                }
            }
            Err(error) => {
                // A connection failure is safe to try on another explicitly
                // supplied fleet endpoint: signed transaction replay is
                // rejected by consensus if the first endpoint committed it.
                failures.push(format!("{address}: {error}"));
            }
        }
    }
    bail!(
        "no supplied PFTL endpoint finalized the reserve packet: {}",
        failures.join("; ")
    )
}

fn resolve_rpc_address(address: &str) -> Result<SocketAddr> {
    anyhow::ensure!(
        !address.contains("://"),
        "rpc_address must be HOST:PORT for the PFTL newline-JSON RPC, not a URL"
    );
    let mut resolved = address
        .to_socket_addrs()
        .with_context(|| format!("resolve PFTL RPC address {address}"))?;
    let endpoint = resolved
        .next()
        .with_context(|| format!("PFTL RPC address did not resolve: {address}"))?;
    anyhow::ensure!(
        resolved.next().is_none(),
        "PFTL RPC address must resolve to exactly one endpoint: {address}"
    );
    Ok(endpoint)
}

fn rpc_roundtrip(
    endpoint: SocketAddr,
    request: &[u8],
    timeout: Duration,
) -> Result<serde_json::Value> {
    let mut stream = TcpStream::connect_timeout(&endpoint, timeout)
        .with_context(|| format!("connect PFTL RPC {endpoint}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(request)?;
    stream.write_all(b"\n")?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut response = Vec::new();
    stream
        .take((MAX_PFTL_RPC_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut response)?;
    anyhow::ensure!(
        response.len() <= MAX_PFTL_RPC_RESPONSE_BYTES,
        "PFTL RPC response exceeds {MAX_PFTL_RPC_RESPONSE_BYTES} bytes"
    );
    anyhow::ensure!(!response.is_empty(), "PFTL RPC returned an empty response");
    serde_json::from_slice(&response).context("decode PFTL RPC response")
}

fn require_accepted_finality(response: &serde_json::Value) -> Result<()> {
    anyhow::ensure!(
        response
            .pointer("/result/finality/confirmed")
            .and_then(serde_json::Value::as_bool)
            == Some(true),
        "PFTL RPC success response does not contain confirmed finality"
    );
    anyhow::ensure!(
        response
            .pointer("/result/finality/receipt/accepted")
            .and_then(serde_json::Value::as_bool)
            == Some(true),
        "PFTL RPC success response does not contain an accepted receipt"
    );
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<T> {
    serde_json::from_slice(&read_bounded(path, MAX_WITNESS_BYTES, "JSON input")?)
        .with_context(|| format!("decode JSON {}", path.display()))
}

fn read_bounded(path: &PathBuf, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "{label} is not a regular file: {}",
        path.display()
    );
    let length = usize::try_from(metadata.len()).context("input length exceeds platform usize")?;
    anyhow::ensure!(
        length <= maximum,
        "{label} exceeds {maximum} bytes: {}",
        path.display()
    );
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
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create new output {}", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("sync {}", path.display()))
}

fn normalize_cbor_bytes(value: &mut serde_cbor::Value) {
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
        serde_cbor::Value::Array(values) => values.iter_mut().for_each(normalize_cbor_bytes),
        serde_cbor::Value::Map(values) => values.values_mut().for_each(normalize_cbor_bytes),
        serde_cbor::Value::Tag(_, value) => normalize_cbor_bytes(value),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn rpc_fixture(response: serde_json::Value) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).unwrap();
            assert!(request.ends_with(b"\n"));
            let decoded: serde_json::Value = serde_json::from_slice(&request).unwrap();
            assert_eq!(
                decoded.get("method").and_then(serde_json::Value::as_str),
                Some("mempool_submit_signed_asset_transaction_finality")
            );
            stream
                .write_all(&serde_json::to_vec(&response).unwrap())
                .unwrap();
        });
        endpoint
    }

    #[test]
    fn packet_rpc_requires_confirmed_accepted_finality() {
        let accepted = serde_json::json!({
            "ok": true,
            "result": {"finality": {"confirmed": true, "receipt": {"accepted": true}}}
        });
        let endpoint = rpc_fixture(accepted);
        let request = serde_json::to_vec(&serde_json::json!({
            "method": "mempool_submit_signed_asset_transaction_finality"
        }))
        .unwrap();
        let response = rpc_roundtrip(endpoint, &request, Duration::from_secs(2)).unwrap();
        require_accepted_finality(&response).unwrap();

        for rejected in [
            serde_json::json!({
                "ok": true,
                "result": {"finality": {"confirmed": false, "receipt": {"accepted": true}}}
            }),
            serde_json::json!({
                "ok": true,
                "result": {"finality": {"confirmed": true, "receipt": {"accepted": false}}}
            }),
        ] {
            assert!(require_accepted_finality(&rejected).is_err());
        }
    }

    #[test]
    fn packet_rpc_address_rejects_url_schemes() {
        assert!(resolve_rpc_address("http://127.0.0.1:28650").is_err());
        assert!(resolve_rpc_address("127.0.0.1:28650").is_ok());
    }

    fn qualified_public_values() -> (NavReservePublicValuesV1, Vec<u8>) {
        let bytes = hex::decode(
            include_str!(
            "../../../../../crates/execution/testdata/nav-reserve-v1-qualified-public-values.hex"
        )
            .trim(),
        )
        .expect("decode qualified public-values fixture");
        let values =
            NavReservePublicValuesV1::decode(&bytes).expect("decode qualified public-values ABI");
        (values, bytes)
    }

    fn packet_template(source_root: String) -> PacketTemplateV1 {
        PacketTemplateV1 {
            schema: "postfiat.reserve_packet_template.v1".to_string(),
            issuer: "pf1111111111111111111111111111111111111111".to_string(),
            submitter: "pf2222222222222222222222222222222222222222".to_string(),
            nav_per_unit: 1,
            circulating_supply: 0,
            source_root,
            attestor_root:
                "cb34590e25db391724491b01795dee8bdbbadba3bba36fb5fc4f96bce1a87fa311426e0b76ce5ff4d775b091d94147df"
                    .to_string(),
            reserve_packet_hash: "55".repeat(48),
            subscription_overlay_source_root: None,
            subscription_overlay_value: 0,
        }
    }

    #[test]
    fn packet_builder_preserves_proof_only_shape() {
        let (values, bytes) = qualified_public_values();
        let template = packet_template(values.source_observation_root.clone());
        let operation = build_packet_operation(template, &values, vec![1], bytes)
            .expect("proof-only packet operation");
        assert_eq!(operation.source_root, values.source_observation_root);
        assert_eq!(operation.verified_net_assets, values.verified_net_assets);
    }

    #[test]
    fn packet_builder_constructs_and_checks_subscription_overlay() {
        let (values, bytes) = qualified_public_values();
        let overlay_root = "0b".repeat(48);
        let overlay_value = 500;
        let (composite_root, total) = nav_reserve_subscription_composite_source_root_v1(
            &values,
            &overlay_root,
            overlay_value,
        )
        .expect("derive composite root");
        let mut template = packet_template(composite_root.clone());
        template.subscription_overlay_source_root = Some(overlay_root);
        template.subscription_overlay_value = overlay_value;
        let operation = build_packet_operation(template, &values, vec![1], bytes.clone())
            .expect("overlay packet operation");
        assert_eq!(operation.source_root, composite_root);
        assert_eq!(operation.verified_net_assets, total);

        let mut mismatched = packet_template("ff".repeat(48));
        mismatched.subscription_overlay_source_root = Some("0b".repeat(48));
        mismatched.subscription_overlay_value = overlay_value;
        assert!(build_packet_operation(mismatched, &values, vec![1], bytes).is_err());
    }

    #[test]
    fn subscription_overlay_fixture_template_builds_expected_packet() {
        let (values, bytes) = qualified_public_values();
        let template: PacketTemplateV1 = serde_json::from_str(include_str!(
            "../../../fixtures/controlled-two-source/packet-template-subscription-overlay.json"
        ))
        .expect("parse subscription overlay packet-template fixture");
        let expected_root = template.source_root.clone();
        let operation = build_packet_operation(template, &values, vec![1], bytes)
            .expect("fixture overlay packet operation");
        assert_eq!(operation.source_root, expected_root);
        assert_eq!(
            operation.verified_net_assets,
            values.verified_net_assets + 500
        );
    }
}
