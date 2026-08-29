use std::{collections::BTreeMap, fs, path::PathBuf, time::Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pfusdc_arc_ingress_program::{verify_arc_ingress_witness_v1, ArcIngressWitnessV1};
use pfusdc_ingress_program::bonded::{
    bonded_ingress_policy_hash_v1, verify_bonded_age_release_witness_v1,
    verify_bonded_confirmation_witness_v1, verify_bonded_ingress_witness_v1,
    PfUsdcBondedAgeReleaseWitnessV1, PfUsdcBondedConfirmationWitnessV1, PfUsdcBondedGuestInputV1,
    PfUsdcBondedIngressPolicyV1, PfUsdcBondedIngressWitnessV1, PfUsdcBondedReversionWitnessV1,
};
use pfusdc_ingress_program::{verify_ingress_witness_v2, PfUsdcIngressProofWitnessV2};
use postfiat_pfusdc_proofs::{verify_checkpoint_witness_v1, verify_egress_witness_v1};
use postfiat_types::{
    vault_bridge_deposit_evidence_root, vault_bridge_deposit_id, vault_bridge_route_binding, PfUsdcCheckpointProofWitnessV1, PfUsdcEgressProgramInputV1,
    PfUsdcEgressProofWitnessV1, PfUsdcIngressPublicValuesV3, VaultBridgeDepositEvidence,
    VaultBridgeRouteProfileV1,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
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
const ARC_INGRESS_ELF: Elf = Elf::Static(include_bytes!(
    "../../../programs/pfusdc-arc-ingress/elf/pfusdc-arc-ingress-program"
));

mod arc_ingress_capture;
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
    /// Derive the ELF hash and vkey for the separately frozen bonded guest.
    BondedProgramInfo {
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Cross-check a bonded policy/profile and print their governed hashes.
    BondedRouteInfo {
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        route_profile: PathBuf,
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
    /// Validate a route profile and derive its canonical hash and route binding.
    RouteProfileInfo {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Validate one canonical vault-deposit evidence row and derive its IDs.
    DepositEvidenceInfo {
        #[arg(long)]
        evidence: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Capture and natively verify one finalized Ethereum/Arbitrum ingress witness.
    IngressCapture(ingress_capture::IngressCaptureArgs),
    /// Capture and natively verify one finalized Arc direct-deposit witness.
    ArcIngressCapture(arc_ingress_capture::ArcIngressCaptureArgs),
    /// Run deterministic security-field mutations against an Arc witness.
    ArcIngressAudit {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Capture one Ethereum-finalized, bonded-assertion ingress witness.
    BondedIngressCapture(ingress_capture::bonded::BondedIngressCaptureArgs),
    /// Capture a newer Ethereum-finalized confirmation for an escrowed mint.
    BondedConfirmationCapture(ingress_capture::bonded::BondedConfirmationCaptureArgs),
    /// Capture proof that a previously bonded assertion lost to a sibling.
    BondedReversionCapture(ingress_capture::bonded::BondedReversionCaptureArgs),
    /// Capture a newer finalized snapshot proving short-age eligibility on the unique live branch.
    BondedAgeReleaseCapture(ingress_capture::bonded::BondedAgeReleaseCaptureArgs),
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
    /// Execute or Groth16-prove the bonded-assertion ingress guest.
    BondedIngress {
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a bonded assertion confirmation update.
    BondedConfirmation {
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a bonded assertion reversion update.
    BondedReversion {
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a bonded assertion age release.
    BondedAgeRelease {
        #[arg(long)]
        elf: PathBuf,
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a canonical Arc ingress witness.
    ArcIngress {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        prove: bool,
    },
    /// Execute or Groth16-prove a canonical PFTL egress witness.
    Egress {
        /// Optional frozen egress ELF. Defaults to the repository-embedded release.
        #[arg(long)]
        elf: Option<PathBuf>,
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
        Command::BondedProgramInfo { elf, output } => bonded_program_info(elf, output).await,
        Command::BondedRouteInfo {
            policy,
            route_profile,
            output,
        } => bonded_route_info(policy, route_profile, output),
        Command::DeploymentManifest { input, output } => manifest::run(input, output),
        Command::RouteProfileInfo { profile, output } => route_profile_info(profile, output),
        Command::DepositEvidenceInfo { evidence, output } => deposit_evidence_info(evidence, output),
        Command::IngressCapture(capture) => ingress_capture::capture(capture).await,
        Command::ArcIngressCapture(capture) => arc_ingress_capture::capture(capture).await,
        Command::ArcIngressAudit { witness, output } => arc_ingress_audit(witness, output),
        Command::BondedIngressCapture(capture) => ingress_capture::bonded::capture(capture).await,
        Command::BondedConfirmationCapture(capture) => {
            ingress_capture::bonded::capture_confirmation(capture).await
        }
        Command::BondedReversionCapture(capture) => {
            ingress_capture::bonded::capture_reversion(capture).await
        }
        Command::BondedAgeReleaseCapture(capture) => {
            ingress_capture::bonded::capture_age_release(capture).await
        }
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
        Command::BondedIngress {
            elf,
            witness,
            output_dir,
            prove,
        } => prove_bonded_ingress(elf, witness, output_dir, prove).await,
        Command::BondedConfirmation {
            elf,
            witness,
            output_dir,
            prove,
        } => prove_bonded_confirmation(elf, witness, output_dir, prove).await,
        Command::BondedReversion {
            elf,
            witness,
            output_dir,
            prove,
        } => prove_bonded_reversion(elf, witness, output_dir, prove).await,
        Command::BondedAgeRelease {
            elf,
            witness,
            output_dir,
            prove,
        } => prove_bonded_age_release(elf, witness, output_dir, prove).await,
        Command::ArcIngress {
            witness,
            output_dir,
            prove,
        } => prove_arc_ingress(witness, output_dir, prove).await,
        Command::Egress {
            elf,
            witness,
            output_dir,
            prove,
        } => prove_egress(elf, witness, output_dir, prove).await,
        Command::Checkpoint {
            witness,
            output_dir,
            prove,
        } => prove_checkpoint(witness, output_dir, prove).await,
    }
}

fn deposit_evidence_info(evidence_path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let bytes = fs::read(&evidence_path)
        .with_context(|| format!("read deposit evidence {}", evidence_path.display()))?;
    let evidence: VaultBridgeDepositEvidence = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode deposit evidence {}", evidence_path.display()))?;
    evidence.validate().map_err(anyhow::Error::msg)?;
    let derived_deposit_id = vault_bridge_deposit_id(&evidence).map_err(anyhow::Error::msg)?;
    let evidence_root =
        vault_bridge_deposit_evidence_root(&evidence).map_err(anyhow::Error::msg)?;
    let document = serde_json::json!({
        "schema": "postfiat.pfusdc.deposit_evidence_info.v1",
        "evidence": evidence_path,
        "derived_deposit_id": derived_deposit_id,
        "evidence_root": evidence_root,
        "source_domain": evidence.source_domain(),
        "finality_ref": evidence.finality_ref(),
    });
    let encoded = serde_json::to_vec_pretty(&document)?;
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &encoded)?;
    }
    println!("{}", String::from_utf8(encoded)?);
    Ok(())
}

fn arc_ingress_audit(witness_path: PathBuf, output: PathBuf) -> Result<()> {
    let bytes = fs::read(&witness_path)
        .with_context(|| format!("read Arc ingress witness {}", witness_path.display()))?;
    let witness: ArcIngressWitnessV1 = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode Arc ingress witness {}", witness_path.display()))?;
    verify_arc_ingress_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("baseline Arc witness rejected: {}", error.code()))?;
    let mut results = Vec::new();
    let mut check = |name: &str, mutated: ArcIngressWitnessV1, expected: &str| -> Result<()> {
        let error = verify_arc_ingress_witness_v1(&mutated)
            .expect_err("security mutation unexpectedly verified");
        anyhow::ensure!(
            error.code() == expected,
            "mutation {name} returned {}, expected {expected}",
            error.code()
        );
        results.push(serde_json::json!({
            "mutation": name,
            "accepted": false,
            "error_code": error.code(),
        }));
        Ok(())
    };

    let mut forged = witness.clone();
    forged.signatures[0].signature[0] ^= 1;
    check("forged_signature", forged, "ARC_INGRESS_INVALID_SIGNATURE")?;

    let mut subquorum = witness.clone();
    subquorum.signatures.truncate(1);
    check("subquorum", subquorum, "ARC_INGRESS_SUB_QUORUM")?;

    let mut receipt = witness.clone();
    let last = receipt
        .encoded_receipt
        .last_mut()
        .context("baseline receipt is empty")?;
    *last ^= 1;
    check("mutated_receipt", receipt, "ARC_INGRESS_RECEIPT_PROOF")?;

    let mut wrong_log = witness.clone();
    wrong_log.amount_atoms = wrong_log
        .amount_atoms
        .checked_add(1)
        .context("amount mutation overflow")?;
    check("wrong_log_fields", wrong_log, "ARC_INGRESS_DEPOSIT_MISMATCH")?;

    let mut stale_set = witness.clone();
    stale_set.validator_set_commitment_in[0] ^= 1;
    check(
        "stale_validator_set_commitment",
        stale_set,
        "ARC_INGRESS_VALIDATOR_SET_COMMITMENT_MISMATCH",
    )?;

    let mut asserted_rotation = witness;
    asserted_rotation.next_validators = asserted_rotation.validators.clone();
    check(
        "unauthenticated_rotation",
        asserted_rotation,
        "ARC_INGRESS_ROTATION_PROOF_UNAVAILABLE",
    )?;

    let report = serde_json::json!({
        "schema": "postfiat.pfusdc.arc_ingress_negative_suite.v1",
        "witness": witness_path,
        "baseline_accepted": true,
        "results": results,
        "replay_scope": "PFTL state transition; deposit_id replay is not a stateless guest property",
    });
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn route_profile_info(profile_path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let bytes = fs::read(&profile_path)
        .with_context(|| format!("read route profile {}", profile_path.display()))?;
    let profile: VaultBridgeRouteProfileV1 = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode route profile {}", profile_path.display()))?;
    profile.validate().map_err(anyhow::Error::msg)?;
    let profile_hash = profile.profile_hash().map_err(anyhow::Error::msg)?;
    let route_binding = vault_bridge_route_binding(&profile_hash, profile.route_epoch)
        .map_err(anyhow::Error::msg)?;
    let document = serde_json::json!({
        "schema": "postfiat.pfusdc.route_profile_info.v1",
        "profile": profile_path,
        "profile_hash": profile_hash,
        "route_binding": route_binding,
    });
    let encoded = serde_json::to_vec_pretty(&document)?;
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &encoded)?;
    }
    println!("{}", String::from_utf8(encoded)?);
    Ok(())
}

async fn prove_arc_ingress(witness_path: PathBuf, output_dir: PathBuf, prove: bool) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read Arc ingress witness {}", witness_path.display()))?;
    let witness: ArcIngressWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode Arc ingress witness {}", witness_path.display()))?;
    let expected = verify_arc_ingress_witness_v1(&witness).map_err(|error| {
        anyhow::anyhow!("native Arc ingress verification failed: {}", error.code())
    })?;
    let expected_public_values = expected.canonical_bytes();
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(serde_cbor::to_vec(&witness).context("encode Arc ingress witness as CBOR")?);
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(ARC_INGRESS_ELF, stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 Arc ingress output differs from native canonical public values"
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
            "schema": "postfiat.pfusdc.arc_ingress_execute_report.v1",
            "witness": witness_path,
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    println!(
        "Arc ingress witness executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    if prove {
        let setup_started = Instant::now();
        let pk = client.setup(ARC_INGRESS_ELF).await?;
        let proof = client.prove(&pk, stdin).groth16().await?;
        client.verify(&proof, pk.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified Arc ingress proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
        fs::write(
            output_dir.join("proof-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "postfiat.pfusdc.arc_ingress_proof_report.v1",
                "program_vkey": pk.verifying_key().bytes32(),
                "proof_mode": "groth16",
                "setup_and_prove_ms": setup_started.elapsed().as_millis(),
                "proof_bytes": proof.bytes().len(),
                "public_values_bytes": proof.public_values.to_vec().len(),
            }))?,
        )?;
        println!(
            "verified Arc ingress Groth16 proof; vkey {}",
            pk.verifying_key().bytes32()
        );
    }
    Ok(())
}

fn bonded_route_info(
    policy_path: PathBuf,
    route_profile_path: PathBuf,
    output: Option<PathBuf>,
) -> Result<()> {
    let policy: PfUsdcBondedIngressPolicyV1 = serde_json::from_slice(
        &fs::read(&policy_path)
            .with_context(|| format!("read bonded policy {}", policy_path.display()))?,
    )
    .with_context(|| format!("decode bonded policy {}", policy_path.display()))?;
    let profile: VaultBridgeRouteProfileV1 = serde_json::from_slice(
        &fs::read(&route_profile_path)
            .with_context(|| format!("read route profile {}", route_profile_path.display()))?,
    )
    .with_context(|| format!("decode route profile {}", route_profile_path.display()))?;
    profile.validate().map_err(anyhow::Error::msg)?;
    let policy_hash = bonded_ingress_policy_hash_v1(&policy);
    anyhow::ensure!(
        profile.verifier_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
        "fast ingress must coexist with the base confirmed Tier-4 route"
    );
    let profile_hash = profile.profile_hash().map_err(anyhow::Error::msg)?;
    let route_binding = vault_bridge_route_binding(&profile_hash, profile.route_epoch)
        .map_err(anyhow::Error::msg)?;
    let document = serde_json::json!({
        "schema": "postfiat.pfusdc.bonded_route_info.v1",
        "profile_hash": profile_hash,
        "route_binding": route_binding,
        "route_id": profile.route_id,
        "route_epoch": profile.route_epoch,
        "bonded_policy_hash": policy_hash,
        "base_confirmed_program_vkey": profile.verifier_program_vkey,
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

async fn bonded_program_info(elf_path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let elf_bytes =
        fs::read(&elf_path).with_context(|| format!("read bonded ELF {}", elf_path.display()))?;
    let elf = Elf::from(elf_bytes.clone());
    let client = ProverClient::from_env().await;
    let pk = client.setup(elf).await?;
    let document = serde_json::json!({
        "schema": "postfiat.pfusdc.bonded_ingress_program_info.v1",
        "sp1_sdk_version": "6.3.1",
        "elf_sha256": hex::encode(Sha256::digest(&elf_bytes)),
        "program_vkey": pk.verifying_key().bytes32(),
        "proof_mode": "groth16",
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

async fn program_info(output: Option<PathBuf>) -> Result<()> {
    let client = ProverClient::from_env().await;
    let ingress = client.setup(INGRESS_ELF).await?;
    let arc_ingress = client.setup(ARC_INGRESS_ELF).await?;
    let egress = client.setup(EGRESS_ELF).await?;
    let document = serde_json::json!({
        "schema": "postfiat.pfusdc.tier4_program_info.v1",
        "sp1_sdk_version": "6.3.1",
        "ingress": {
            "elf_sha256": hex::encode(Sha256::digest(&*INGRESS_ELF)),
            "program_vkey": ingress.verifying_key().bytes32(),
        },
        "arc_ingress": {
            "elf_sha256": hex::encode(Sha256::digest(&*ARC_INGRESS_ELF)),
            "program_vkey": arc_ingress.verifying_key().bytes32(),
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
        let actual =
            PfUsdcIngressPublicValuesV3::from_canonical_bytes(&executed).map_err(|error| {
                anyhow::anyhow!(
                    "decode SP1 ingress output: {error}; bytes={}, hex={}",
                    executed.len(),
                    hex::encode(&executed)
                )
            })?;
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
                "proof_mode": "groth16",
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

async fn prove_bonded_ingress(
    elf_path: PathBuf,
    witness_path: PathBuf,
    output_dir: PathBuf,
    prove: bool,
) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let elf_bytes =
        fs::read(&elf_path).with_context(|| format!("read bonded ELF {}", elf_path.display()))?;
    let elf = Elf::from(elf_bytes);
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read bonded witness {}", witness_path.display()))?;
    let witness: PfUsdcBondedIngressWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode bonded witness {}", witness_path.display()))?;
    let expected = verify_bonded_ingress_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native bonded witness verification failed: {error}"))?;
    let expected_public_values = expected
        .canonical_bytes_without_commitment()
        .map_err(|error| anyhow::anyhow!("encode bonded public values: {error}"))?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(
        serde_cbor::to_vec(&PfUsdcBondedGuestInputV1::Ingress(witness))
            .context("encode bonded witness as CBOR")?,
    );
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(elf.clone(), stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 bonded output differs from native canonical public values"
    );
    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat.pfusdc.bonded_ingress_execute_report.v1",
            "witness": witness_path,
            "elf_sha256": hex::encode(Sha256::digest(fs::read(&elf_path)?)),
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    println!(
        "bonded ingress witness executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    if prove {
        let setup_started = Instant::now();
        let pk = client.setup(elf).await?;
        let proof = client.prove(&pk, stdin).groth16().await?;
        client.verify(&proof, pk.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified bonded proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
        fs::write(
            output_dir.join("proof-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "postfiat.pfusdc.bonded_ingress_proof_report.v1",
                "program_vkey": pk.verifying_key().bytes32(),
                "proof_mode": "groth16",
                "setup_and_prove_ms": setup_started.elapsed().as_millis(),
                "proof_bytes": proof.bytes().len(),
                "public_values_bytes": proof.public_values.to_vec().len(),
            }))?,
        )?;
        println!(
            "verified bonded Groth16 proof; vkey {}",
            pk.verifying_key().bytes32()
        );
    }
    Ok(())
}

async fn prove_bonded_confirmation(
    elf_path: PathBuf,
    witness_path: PathBuf,
    output_dir: PathBuf,
    prove: bool,
) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let elf_bytes =
        fs::read(&elf_path).with_context(|| format!("read bonded ELF {}", elf_path.display()))?;
    let elf = Elf::from(elf_bytes);
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read confirmation witness {}", witness_path.display()))?;
    let witness: PfUsdcBondedConfirmationWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode confirmation witness {}", witness_path.display()))?;
    let expected = verify_bonded_confirmation_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native confirmation verification failed: {error}"))?;
    let expected_public_values = expected
        .canonical_bytes_without_commitment()
        .map_err(|error| anyhow::anyhow!("encode confirmation public values: {error}"))?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(
        serde_cbor::to_vec(&PfUsdcBondedGuestInputV1::Confirmation(witness))
            .context("encode confirmation witness as CBOR")?,
    );
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(elf.clone(), stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 confirmation output differs from native public values"
    );
    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat.pfusdc.bonded_confirmation_execute_report.v1",
            "witness": witness_path,
            "elf_sha256": hex::encode(Sha256::digest(fs::read(&elf_path)?)),
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    println!(
        "bonded confirmation executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    if prove {
        let setup_started = Instant::now();
        let pk = client.setup(elf).await?;
        let proof = client.prove(&pk, stdin).groth16().await?;
        client.verify(&proof, pk.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified confirmation proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
        fs::write(
            output_dir.join("proof-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "postfiat.pfusdc.bonded_confirmation_proof_report.v1",
                "program_vkey": pk.verifying_key().bytes32(),
                "proof_mode": "groth16",
                "setup_and_prove_ms": setup_started.elapsed().as_millis(),
                "proof_bytes": proof.bytes().len(),
                "public_values_bytes": proof.public_values.to_vec().len(),
            }))?,
        )?;
        println!(
            "verified bonded confirmation Groth16 proof; vkey {}",
            pk.verifying_key().bytes32()
        );
    }
    Ok(())
}

async fn prove_bonded_age_release(
    elf_path: PathBuf,
    witness_path: PathBuf,
    output_dir: PathBuf,
    prove: bool,
) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let elf_bytes =
        fs::read(&elf_path).with_context(|| format!("read bonded ELF {}", elf_path.display()))?;
    let elf = Elf::from(elf_bytes);
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read age-release witness {}", witness_path.display()))?;
    let witness: PfUsdcBondedAgeReleaseWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode age-release witness {}", witness_path.display()))?;
    let expected = verify_bonded_age_release_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native age-release verification failed: {error}"))?;
    let expected_public_values = expected
        .canonical_bytes_without_commitment()
        .map_err(|error| anyhow::anyhow!("encode age-release public values: {error}"))?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(
        serde_cbor::to_vec(&PfUsdcBondedGuestInputV1::AgeRelease(witness))
            .context("encode age-release witness as CBOR")?,
    );
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(elf.clone(), stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 age-release output differs from native public values"
    );
    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat.pfusdc.bonded_age_release_execute_report.v1",
            "witness": witness_path,
            "elf_sha256": hex::encode(Sha256::digest(fs::read(&elf_path)?)),
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    println!(
        "bonded age release executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    if prove {
        let setup_started = Instant::now();
        let pk = client.setup(elf).await?;
        let proof = client.prove(&pk, stdin).groth16().await?;
        client.verify(&proof, pk.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified age-release proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
        fs::write(
            output_dir.join("proof-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "postfiat.pfusdc.bonded_age_release_proof_report.v1",
                "program_vkey": pk.verifying_key().bytes32(),
                "proof_mode": "groth16",
                "setup_and_prove_ms": setup_started.elapsed().as_millis(),
                "proof_bytes": proof.bytes().len(),
                "public_values_bytes": proof.public_values.to_vec().len(),
            }))?,
        )?;
        println!(
            "verified bonded age-release Groth16 proof; vkey {}",
            pk.verifying_key().bytes32()
        );
    }
    Ok(())
}

async fn prove_bonded_reversion(
    elf_path: PathBuf,
    witness_path: PathBuf,
    output_dir: PathBuf,
    prove: bool,
) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let elf_bytes =
        fs::read(&elf_path).with_context(|| format!("read bonded ELF {}", elf_path.display()))?;
    let elf = Elf::from(elf_bytes);
    let witness_bytes = fs::read(&witness_path)
        .with_context(|| format!("read reversion witness {}", witness_path.display()))?;
    let witness: PfUsdcBondedReversionWitnessV1 = serde_json::from_slice(&witness_bytes)
        .with_context(|| format!("decode reversion witness {}", witness_path.display()))?;
    let expected = pfusdc_ingress_program::bonded::verify_bonded_reversion_witness_v1(&witness)
        .map_err(|error| anyhow::anyhow!("native reversion verification failed: {error}"))?;
    let expected_public_values = expected
        .canonical_bytes_without_commitment()
        .map_err(|error| anyhow::anyhow!("encode reversion public values: {error}"))?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(
        serde_cbor::to_vec(&PfUsdcBondedGuestInputV1::Reversion(witness))
            .context("encode reversion witness as CBOR")?,
    );
    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let (executed_public_values, report) = client.execute(elf.clone(), stdin.clone()).await?;
    let executed = executed_public_values.to_vec();
    anyhow::ensure!(
        executed == expected_public_values,
        "SP1 reversion output differs from native public values"
    );
    fs::create_dir_all(&output_dir)?;
    fs::write(output_dir.join("public-values.bin"), &executed)?;
    fs::write(
        output_dir.join("execute-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "postfiat.pfusdc.bonded_reversion_execute_report.v1",
            "witness": witness_path,
            "elf_sha256": hex::encode(Sha256::digest(fs::read(&elf_path)?)),
            "elapsed_ms": started.elapsed().as_millis(),
            "instruction_count": report.total_instruction_count(),
            "public_values_bytes": executed.len(),
        }))?,
    )?;
    println!(
        "bonded reversion executed: {} cycles in {} ms",
        report.total_instruction_count(),
        started.elapsed().as_millis()
    );
    if prove {
        let setup_started = Instant::now();
        let pk = client.setup(elf).await?;
        let proof = client.prove(&pk, stdin).groth16().await?;
        client.verify(&proof, pk.verifying_key(), None)?;
        anyhow::ensure!(
            proof.public_values.to_vec() == expected_public_values,
            "verified reversion proof contains unexpected public values"
        );
        fs::write(output_dir.join("proof.bin"), bincode::serialize(&proof)?)?;
        fs::write(output_dir.join("proof-calldata.bin"), proof.bytes())?;
        fs::write(
            output_dir.join("proof-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "postfiat.pfusdc.bonded_reversion_proof_report.v1",
                "program_vkey": pk.verifying_key().bytes32(),
                "proof_mode": "groth16",
                "setup_and_prove_ms": setup_started.elapsed().as_millis(),
                "proof_bytes": proof.bytes().len(),
                "public_values_bytes": proof.public_values.to_vec().len(),
            }))?,
        )?;
        println!(
            "verified bonded reversion Groth16 proof; vkey {}",
            pk.verifying_key().bytes32()
        );
    }
    Ok(())
}

async fn prove_egress(
    elf_path: Option<PathBuf>,
    witness_path: PathBuf,
    output_dir: PathBuf,
    prove: bool,
) -> Result<()> {
    #[cfg(debug_assertions)]
    if prove {
        anyhow::bail!("Groth16 proving requires a --release build");
    }
    let elf_bytes = elf_path
        .as_ref()
        .map(|path| fs::read(path).with_context(|| format!("read egress ELF {}", path.display())))
        .transpose()?;
    let elf = elf_bytes
        .map(Elf::from)
        .unwrap_or_else(|| EGRESS_ELF.clone());
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
    let (executed_public_values, report) = client.execute(elf.clone(), stdin.clone()).await?;
    let cycle_tracker = report
        .cycle_tracker
        .iter()
        .map(|(label, cycles)| (label.clone(), *cycles))
        .collect::<BTreeMap<_, _>>();
    let invocation_tracker = report
        .invocation_tracker
        .iter()
        .map(|(label, invocations)| (label.clone(), *invocations))
        .collect::<BTreeMap<_, _>>();
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
            "cycle_tracker": cycle_tracker,
            "invocation_tracker": invocation_tracker,
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
        let pk = client.setup(elf).await?;
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
                "proof_mode": "groth16",
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
