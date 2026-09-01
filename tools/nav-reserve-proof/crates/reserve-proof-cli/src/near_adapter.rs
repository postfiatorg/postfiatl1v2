use std::{path::PathBuf, time::Duration};

use alloy_primitives::B256;
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use clap::Subcommand;
use reqwest::{blocking::Client, redirect::Policy};
use reserve_proof_types::{
    bft_checkpoint::{
        BftCheckpointCommitteeV1, BftSourceCheckpointCertificateV1, BftSourceCheckpointV1,
    },
    near_receipt::{
        decode_near_snapshot_payload, near_block_hash_from_lite, near_block_merkle_root_from_proof,
        near_head_block_hash, near_outcome_root, near_owner_authorization_statement_v1,
        near_receipt_evidence_commitment_v1, near_snapshot_event_from_logs,
        near_success_value_payload, verify_near_receipt_quantity_proof_v1, NearFullBlockHeader,
        NearHeadBlock, NearLightClientProof, NearReceiptPolicyV1, NearReceiptQuantityProofV1,
        NearReceiptVerifyContextV1, NEAR_CHECKPOINT_KIND_V1, NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1,
    },
    verify_observation_evidence, EvidenceDimensionV1, ReserveProofContextV1, SourceEvidenceV1,
    SourceManifestEntryV1, SourceObservationV1, TrustClassV1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::checkpoint_signing::{maybe_sign_reproduced_checkpoint, CheckpointSigningArgs};
use crate::evm_adapter::{
    decode_hex, load_source, read_json, rpc_call, validate_rpc_url, write_new,
};

const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const SNAPSHOT_GAS: &str = "60000000000000";
const MAX_NEAR_CODE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub enum NearCommand {
    /// Emit an unsigned zero-deposit NEAR function-call request for the public
    /// reader. Sign and submit it with an external NEAR wallet.
    SnapshotRequest {
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        account_id: String,
        #[arg(long)]
        salt: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Query an exact finalized NEAR head and emit the checkpoint candidate
    /// each validator must independently reproduce before signing.
    CheckpointCandidate {
        #[arg(long)]
        pftl_genesis_hash: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        source_height: u64,
        /// Common finalized head pinned by the checkpoint coordinator. Every
        /// validator independently requires its live finalized head to be at
        /// least this value before signing.
        #[arg(long)]
        observed_source_head: u64,
        #[arg(long)]
        minimum_depth: u32,
        #[arg(long)]
        pftl_observation_height: u64,
        #[arg(long)]
        committee: PathBuf,
        #[arg(long)]
        rpc_url: String,
        #[arg(long)]
        output: PathBuf,
        #[command(flatten)]
        signing: CheckpointSigningArgs,
    },
    /// Fetch and verify the callback light-client proof, write a proof with a
    /// zero signature placeholder, and emit the exact owner statement.
    Prepare {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        checkpoint_certificate: PathBuf,
        #[arg(long)]
        account_id: String,
        #[arg(long)]
        receipt_id: String,
        #[arg(long)]
        salt: String,
        #[arg(long)]
        rpc_url: String,
        #[arg(long)]
        proof_output: PathBuf,
        #[arg(long)]
        owner_statement_output: PathBuf,
    },
    /// Attach the owner signature and emit a quantity-verified draft for a
    /// separate cryptographic valuation adapter. The draft is intentionally
    /// not a complete source observation and cannot pass valuation checks.
    CollectQuantity {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        prepared_proof: PathBuf,
        #[arg(long)]
        ownership_signature: String,
        #[arg(long)]
        disclosure_commitment: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Attach the external owner signature, verify the complete quantity
    /// proof and separate valuation evidence, then write the observation.
    Collect {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        prepared_proof: PathBuf,
        #[arg(long)]
        ownership_signature: String,
        #[arg(long)]
        valuation_evidence: PathBuf,
        #[arg(long)]
        gross_assets: u64,
        #[arg(long, default_value_t = 0)]
        total_liabilities: u64,
        #[arg(long)]
        disclosure_commitment: String,
        #[arg(long)]
        output: PathBuf,
    },
}

pub fn run(command: NearCommand) -> Result<()> {
    match command {
        NearCommand::SnapshotRequest {
            policy,
            account_id,
            salt,
            output,
        } => snapshot_request(policy, account_id, salt, output),
        NearCommand::CheckpointCandidate {
            pftl_genesis_hash,
            policy,
            source_height,
            observed_source_head,
            minimum_depth,
            pftl_observation_height,
            committee,
            rpc_url,
            output,
            signing,
        } => checkpoint_candidate(CheckpointCandidateArgs {
            pftl_genesis_hash,
            policy,
            source_height,
            observed_source_head,
            minimum_depth,
            pftl_observation_height,
            committee,
            rpc_url,
            output,
            signing,
        }),
        NearCommand::Prepare {
            manifest,
            context,
            source_id,
            policy,
            checkpoint_certificate,
            account_id,
            receipt_id,
            salt,
            rpc_url,
            proof_output,
            owner_statement_output,
        } => prepare(PrepareArgs {
            manifest,
            context,
            source_id,
            policy,
            checkpoint_certificate,
            account_id,
            receipt_id,
            salt,
            rpc_url,
            proof_output,
            owner_statement_output,
        }),
        NearCommand::CollectQuantity {
            manifest,
            context,
            source_id,
            prepared_proof,
            ownership_signature,
            disclosure_commitment,
            output,
        } => collect_quantity(CollectQuantityArgs {
            manifest,
            context,
            source_id,
            prepared_proof,
            ownership_signature,
            disclosure_commitment,
            output,
        }),
        NearCommand::Collect {
            manifest,
            context,
            source_id,
            prepared_proof,
            ownership_signature,
            valuation_evidence,
            gross_assets,
            total_liabilities,
            disclosure_commitment,
            output,
        } => collect(CollectArgs {
            manifest,
            context,
            source_id,
            prepared_proof,
            ownership_signature,
            valuation_evidence,
            gross_assets,
            total_liabilities,
            disclosure_commitment,
            output,
        }),
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct NearSnapshotRequestV1 {
    schema: &'static str,
    receiver_id: String,
    method_name: &'static str,
    args_base64: String,
    gas: &'static str,
    deposit_yocto: &'static str,
    account_id: String,
    pool_id: String,
    salt_base64: String,
    reader_code_hash: String,
    pool_code_hash: String,
}

fn snapshot_request(
    policy_path: PathBuf,
    account_id: String,
    salt: String,
    output: PathBuf,
) -> Result<()> {
    let policy: NearReceiptPolicyV1 = read_json(&policy_path)?;
    policy
        .validate()
        .map_err(|error| anyhow!("NEAR policy is invalid: {error:?}"))?;
    policy
        .reserve_owner_commitment(&account_id)
        .map_err(|error| anyhow!("NEAR reserve owner is invalid: {error:?}"))?;
    let salt = decode_hex("salt", &salt, 32)?;
    anyhow::ensure!(salt.iter().any(|byte| *byte != 0), "salt cannot be zero");
    let salt_base64 = BASE64.encode(&salt);
    let args = serde_json::to_vec(&serde_json::json!({
        "pool_id": policy.pool_id,
        "account_id": account_id,
        "salt": salt_base64,
    }))?;
    let request = NearSnapshotRequestV1 {
        schema: "postfiat.reserve_near_snapshot_request.v1",
        receiver_id: policy.reader_account_id,
        method_name: "snapshot",
        args_base64: BASE64.encode(args),
        gas: SNAPSHOT_GAS,
        deposit_yocto: "0",
        account_id,
        pool_id: policy.pool_id,
        salt_base64,
        reader_code_hash: policy.reader_code_hash,
        pool_code_hash: policy.pool_code_hash,
    };
    write_new(&output, &serde_json::to_vec_pretty(&request)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_near_snapshot_request_report.v1",
            "output": output,
            "receiver_id": request.receiver_id,
            "method_name": request.method_name,
            "account_id": request.account_id,
            "pool_id": request.pool_id,
            "next_required_check": "sign and submit this exact zero-deposit function call with an external NEAR wallet; retain the callback receipt id",
        }))?
    );
    Ok(())
}

struct CheckpointCandidateArgs {
    pftl_genesis_hash: String,
    policy: PathBuf,
    source_height: u64,
    observed_source_head: u64,
    minimum_depth: u32,
    pftl_observation_height: u64,
    committee: PathBuf,
    rpc_url: String,
    output: PathBuf,
    signing: CheckpointSigningArgs,
}

fn checkpoint_candidate(args: CheckpointCandidateArgs) -> Result<()> {
    validate_lower_hex("pftl_genesis_hash", &args.pftl_genesis_hash, 48)?;
    anyhow::ensure!(
        args.source_height > 0
            && args.observed_source_head > 0
            && args.minimum_depth > 0
            && args.pftl_observation_height > 0,
        "checkpoint heights and depth must be nonzero"
    );
    let policy: NearReceiptPolicyV1 = read_json(&args.policy)?;
    policy
        .validate()
        .map_err(|error| anyhow!("NEAR policy is invalid: {error:?}"))?;
    let committee: BftCheckpointCommitteeV1 = read_json(&args.committee)?;
    let committee_root = committee.root().map_err(anyhow::Error::msg)?;
    policy
        .commitment(&committee_root)
        .map_err(|error| anyhow!("NEAR policy commitment failed: {error:?}"))?;
    let client = rpc_client()?;
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    let final_head = fetch_block(&client, &rpc_url, serde_json::json!({"finality": "final"}))?;
    let required_head = args
        .source_height
        .checked_add(u64::from(args.minimum_depth))
        .context("NEAR checkpoint depth overflows")?;
    anyhow::ensure!(
        args.observed_source_head >= required_head,
        "pinned NEAR observation head does not establish the required depth"
    );
    anyhow::ensure!(
        final_head.head_height >= args.observed_source_head,
        "live finalized NEAR head is behind the pinned observation head"
    );
    let source_head = fetch_block(
        &client,
        &rpc_url,
        serde_json::json!({"block_id": args.source_height}),
    )?;
    anyhow::ensure!(
        source_head.head_height == args.source_height,
        "NEAR RPC substituted a different source height"
    );
    verify_code_hashes(&client, &rpc_url, &policy, &source_head)?;
    let head_root = decode_near_hash(
        "NEAR head block merkle root",
        &source_head.head_block_merkle_root,
    )?;
    let head_hash = decode_near_hash("NEAR head hash", &source_head.head_hash)?;
    let checkpoint = BftSourceCheckpointV1 {
        pftl_genesis_hash: args.pftl_genesis_hash,
        checkpoint_kind: NEAR_CHECKPOINT_KIND_V1.to_string(),
        source_domain: policy.source_domain.clone(),
        source_height: source_head.head_height,
        source_timestamp_ms: source_head.header.timestamp / 1_000_000,
        source_block_hash: B256::from(head_hash),
        source_state_commitment: policy
            .source_state_commitment(&head_root)
            .map_err(|error| anyhow!("NEAR source commitment failed: {error:?}"))?,
        observed_source_head: args.observed_source_head,
        minimum_depth: args.minimum_depth,
        pftl_observation_height: args.pftl_observation_height,
        committee_epoch: committee.epoch,
        committee_root,
    };
    checkpoint.canonical_bytes().map_err(anyhow::Error::msg)?;
    maybe_sign_reproduced_checkpoint(&checkpoint, &committee, &args.signing)?;
    write_new(&args.output, &serde_json::to_vec_pretty(&checkpoint)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_near_checkpoint_candidate.v1",
            "output": args.output,
            "source_height": checkpoint.source_height,
            "source_block_hash": source_head.head_hash,
            "block_merkle_root": source_head.head_block_merkle_root,
            "reader_code_hash": policy.reader_code_hash,
            "pool_code_hash": policy.pool_code_hash,
            "observed_source_head": checkpoint.observed_source_head,
            "live_finalized_head": final_head.head_height,
            "minimum_depth": checkpoint.minimum_depth,
            "next_required_check": "each validator independently checks this exact head, both code hashes, and depth before signing",
        }))?
    );
    Ok(())
}

struct PrepareArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    checkpoint_certificate: PathBuf,
    account_id: String,
    receipt_id: String,
    salt: String,
    rpc_url: String,
    proof_output: PathBuf,
    owner_statement_output: PathBuf,
}

fn prepare(args: PrepareArgs) -> Result<()> {
    anyhow::ensure!(
        !args.proof_output.exists() && !args.owner_statement_output.exists(),
        "refusing to overwrite NEAR proof or owner statement output"
    );
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: NearReceiptPolicyV1 = read_json(&args.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    validate_manifest_entry(
        &entry,
        &policy,
        &certificate,
        &args.account_id,
        &context.pftl_genesis_hash,
    )?;
    let salt = decode_hex("salt", &args.salt, 32)?;
    anyhow::ensure!(salt.iter().any(|byte| *byte != 0), "salt cannot be zero");
    decode_near_hash("receipt_id", &args.receipt_id)?;
    let checkpoint = &certificate.checkpoint;
    anyhow::ensure!(
        checkpoint.pftl_observation_height >= context.observation_not_before
            && checkpoint.pftl_observation_height <= context.observation_not_after,
        "NEAR checkpoint PFTL height is outside the observation interval"
    );
    let client = rpc_client()?;
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    let current_final = fetch_block(&client, &rpc_url, serde_json::json!({"finality": "final"}))?;
    anyhow::ensure!(
        current_final.head_height >= checkpoint.observed_source_head,
        "NEAR RPC final head is behind the certified observation head"
    );
    let head = fetch_block(
        &client,
        &rpc_url,
        serde_json::json!({"block_id": bs58::encode(checkpoint.source_block_hash).into_string()}),
    )?;
    validate_head_against_checkpoint(&head, &policy, checkpoint)?;
    verify_code_hashes(&client, &rpc_url, &policy, &head)?;
    let proof: NearLightClientProof = rpc_call(
        &client,
        &rpc_url,
        "EXPERIMENTAL_light_client_proof",
        serde_json::json!({
            "type": "receipt",
            "receipt_id": args.receipt_id,
            "receiver_id": policy.reader_account_id,
            "light_client_head": head.head_hash,
        }),
    )?;
    anyhow::ensure!(
        proof.outcome_proof.id == args.receipt_id,
        "NEAR RPC substituted a different receipt proof"
    );
    let payload = near_success_value_payload(&proof.outcome_proof.outcome.status)
        .map_err(|error| anyhow!("NEAR callback has no SuccessValue: {error:?}"))?;
    let event = near_snapshot_event_from_logs(&proof.outcome_proof.outcome.logs, &policy)
        .map_err(|error| anyhow!("NEAR snapshot event failed: {error:?}"))?;
    anyhow::ensure!(
        event.payload == payload,
        "NEAR event and callback payload differ"
    );
    let decoded = decode_near_snapshot_payload(&payload)
        .map_err(|error| anyhow!("NEAR snapshot payload failed: {error:?}"))?;
    anyhow::ensure!(
        decoded.account_id == args.account_id
            && decoded.pool_id == policy.pool_id
            && decoded.salt.as_slice() == salt.as_slice(),
        "NEAR snapshot account, pool, or salt mismatch"
    );
    validate_light_proof_foundations(&proof, &head)?;
    let witness = NearReceiptQuantityProofV1 {
        policy,
        checkpoint_certificate: certificate,
        account_id: args.account_id,
        ownership_signature: vec![0; 64],
        commitment: event.commitment,
        salt,
        payload,
        proof,
        head,
    };
    let empty_evidence_commitment = "00".repeat(48);
    let verify_context = verification_context(
        &context,
        &entry,
        witness
            .checkpoint_certificate
            .checkpoint
            .pftl_observation_height,
        &empty_evidence_commitment,
    );
    let statement = near_owner_authorization_statement_v1(&witness, &verify_context)
        .map_err(|error| anyhow!("NEAR owner statement failed: {error:?}"))?;
    write_new(&args.proof_output, &serde_json::to_vec_pretty(&witness)?)?;
    write_new(&args.owner_statement_output, &statement)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_near_prepared_proof.v1",
            "proof_output": args.proof_output,
            "owner_statement_output": args.owner_statement_output,
            "receipt_id": args.receipt_id,
            "proven_block_hash": witness.proof.outcome_proof.block_hash,
            "head_hash": witness.head.head_hash,
            "staked_yocto": decoded.staked_yocto.to_string(),
            "unstaked_yocto": decoded.unstaked_yocto.to_string(),
            "next_required_check": "the reserve owner signs owner_statement_output with the policy-pinned Ed25519 key; no private key enters this CLI",
        }))?
    );
    Ok(())
}

struct CollectArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    prepared_proof: PathBuf,
    ownership_signature: String,
    valuation_evidence: PathBuf,
    gross_assets: u64,
    total_liabilities: u64,
    disclosure_commitment: String,
    output: PathBuf,
}

struct CollectQuantityArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    prepared_proof: PathBuf,
    ownership_signature: String,
    disclosure_commitment: String,
    output: PathBuf,
}

fn collect_quantity(args: CollectQuantityArgs) -> Result<()> {
    validate_lower_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let mut proof: NearReceiptQuantityProofV1 = read_json(&args.prepared_proof)?;
    validate_manifest_entry(
        &entry,
        &proof.policy,
        &proof.checkpoint_certificate,
        &proof.account_id,
        &context.pftl_genesis_hash,
    )?;
    proof.ownership_signature = decode_hex("ownership_signature", &args.ownership_signature, 64)?;
    let evidence_commitment = near_receipt_evidence_commitment_v1(&proof)
        .map_err(|error| anyhow!("NEAR evidence commitment failed: {error:?}"))?;
    let observed_at = proof
        .checkpoint_certificate
        .checkpoint
        .pftl_observation_height;
    anyhow::ensure!(
        observed_at >= context.observation_not_before
            && observed_at <= context.observation_not_after,
        "NEAR checkpoint PFTL height is outside the observation interval"
    );
    let quantity_evidence = SourceEvidenceV1::NearReceiptQuantity {
        evidence_commitment: evidence_commitment.clone(),
        proof: Box::new(proof),
    };
    // A quantity draft uses its quantity evidence as a deliberately invalid
    // valuation placeholder. The Chainlink collector replaces it and verifies
    // both dimensions before emitting a complete observation.
    let observation = SourceObservationV1 {
        source_id: entry.source_id.clone(),
        observed_at_block: observed_at,
        gross_assets: 0,
        total_liabilities: 0,
        quantity_evidence: quantity_evidence.clone(),
        valuation_evidence: quantity_evidence,
        disclosure_commitment: args.disclosure_commitment,
    };
    verify_observation_evidence(
        &context,
        &entry,
        &observation,
        EvidenceDimensionV1::Quantity,
    )
    .map_err(anyhow::Error::msg)?;
    write_new(&args.output, &serde_json::to_vec_pretty(&observation)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_near_quantity_draft.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "pftl_observation_height": observation.observed_at_block,
            "quantity_verified": true,
            "valuation_verified": false,
            "next_required_check": "replace the valuation placeholder with a registered cryptographic valuation proof",
            "evidence_commitment": evidence_commitment,
        }))?
    );
    Ok(())
}

fn collect(args: CollectArgs) -> Result<()> {
    anyhow::ensure!(
        args.total_liabilities <= args.gross_assets,
        "total liabilities exceed gross assets"
    );
    validate_lower_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let mut proof: NearReceiptQuantityProofV1 = read_json(&args.prepared_proof)?;
    validate_manifest_entry(
        &entry,
        &proof.policy,
        &proof.checkpoint_certificate,
        &proof.account_id,
        &context.pftl_genesis_hash,
    )?;
    proof.ownership_signature = decode_hex("ownership_signature", &args.ownership_signature, 64)?;
    let evidence_commitment = near_receipt_evidence_commitment_v1(&proof)
        .map_err(|error| anyhow!("NEAR evidence commitment failed: {error:?}"))?;
    let observed_at = proof
        .checkpoint_certificate
        .checkpoint
        .pftl_observation_height;
    anyhow::ensure!(
        observed_at >= context.observation_not_before
            && observed_at <= context.observation_not_after,
        "NEAR checkpoint PFTL height is outside the observation interval"
    );
    let verify_context = verification_context(&context, &entry, observed_at, &evidence_commitment);
    let verified = verify_near_receipt_quantity_proof_v1(&proof, &verify_context)
        .map_err(|error| anyhow!("NEAR quantity proof failed: {error:?}"))?;
    let valuation_evidence: SourceEvidenceV1 = read_json(&args.valuation_evidence)?;
    let observation = SourceObservationV1 {
        source_id: entry.source_id.clone(),
        observed_at_block: observed_at,
        gross_assets: args.gross_assets,
        total_liabilities: args.total_liabilities,
        quantity_evidence: SourceEvidenceV1::NearReceiptQuantity {
            evidence_commitment: evidence_commitment.clone(),
            proof: Box::new(proof),
        },
        valuation_evidence,
        disclosure_commitment: args.disclosure_commitment,
    };
    verify_observation_evidence(
        &context,
        &entry,
        &observation,
        EvidenceDimensionV1::Quantity,
    )
    .map_err(anyhow::Error::msg)?;
    verify_observation_evidence(
        &context,
        &entry,
        &observation,
        EvidenceDimensionV1::Valuation,
    )
    .map_err(anyhow::Error::msg)?;
    write_new(&args.output, &serde_json::to_vec_pretty(&observation)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_near_collection.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "pftl_observation_height": observation.observed_at_block,
            "head_hash": verified.head_block_hash,
            "proven_block_hash": verified.proven_block_hash,
            "staked_yocto": verified.staked_yocto.to_string(),
            "unstaked_yocto": verified.unstaked_yocto.to_string(),
            "total_yocto": verified.total_yocto.to_string(),
            "gross_assets": observation.gross_assets,
            "total_liabilities": observation.total_liabilities,
            "quantity_trust": "cryptographic_bft_checkpoint_near_receipt",
            "valuation_trust": format!("{:?}", entry.valuation_evidence_class).to_lowercase(),
            "evidence_commitment": evidence_commitment,
        }))?
    );
    Ok(())
}

fn validate_manifest_entry(
    entry: &SourceManifestEntryV1,
    policy: &NearReceiptPolicyV1,
    certificate: &BftSourceCheckpointCertificateV1,
    account_id: &str,
    pftl_genesis_hash: &str,
) -> Result<()> {
    policy
        .validate()
        .map_err(|error| anyhow!("NEAR policy is invalid: {error:?}"))?;
    certificate.verify().map_err(anyhow::Error::msg)?;
    let checkpoint = &certificate.checkpoint;
    anyhow::ensure!(
        checkpoint.pftl_genesis_hash == pftl_genesis_hash
            && checkpoint.checkpoint_kind == NEAR_CHECKPOINT_KIND_V1
            && checkpoint.source_domain == policy.source_domain,
        "NEAR checkpoint does not match genesis, kind, or policy"
    );
    let committee_root = certificate.committee.root().map_err(anyhow::Error::msg)?;
    let policy_commitment = policy
        .commitment(&committee_root)
        .map_err(|error| anyhow!("NEAR policy commitment failed: {error:?}"))?;
    let owner_commitment = policy
        .reserve_owner_commitment(account_id)
        .map_err(|error| anyhow!("NEAR reserve owner failed: {error:?}"))?;
    anyhow::ensure!(
        entry.adapter_kind == NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1
            && entry.adapter_schema_version == 1
            && entry.quantity_evidence_class == TrustClassV1::Cryptographic,
        "source does not use the cryptographic NEAR receipt adapter schema 1"
    );
    anyhow::ensure!(
        entry.source_domain == policy.source_domain
            && entry.asset_or_position_id == policy.position_id
            && entry.reserve_owner_commitment == owner_commitment
            && entry.quantity_verifier_commitment == policy_commitment,
        "NEAR manifest identity, owner, or policy commitment mismatch"
    );
    Ok(())
}

fn verification_context<'a>(
    context: &'a ReserveProofContextV1,
    entry: &'a SourceManifestEntryV1,
    observed_at: u64,
    evidence_commitment: &'a str,
) -> NearReceiptVerifyContextV1<'a> {
    NearReceiptVerifyContextV1 {
        pftl_genesis_hash: &context.pftl_genesis_hash,
        nav_asset_id: &context.nav_asset_id,
        proof_profile_id: &context.proof_profile_id,
        valuation_policy_hash: &context.valuation_policy_hash,
        source_manifest_hash: &context.source_manifest_hash,
        source_id: &entry.source_id,
        source_domain: &entry.source_domain,
        asset_or_position_id: &entry.asset_or_position_id,
        reserve_owner_commitment: &entry.reserve_owner_commitment,
        quantity_verifier_commitment: &entry.quantity_verifier_commitment,
        observed_at_pftl_height: observed_at,
        expected_evidence_commitment: evidence_commitment,
    }
}

fn rpc_client() -> Result<Client> {
    Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()
        .context("build NEAR RPC client")
}

fn fetch_block(client: &Client, rpc_url: &reqwest::Url, params: Value) -> Result<NearHeadBlock> {
    let value: Value = rpc_call(client, rpc_url, "block", params)?;
    let header: NearFullBlockHeader = serde_json::from_value(
        value
            .get("header")
            .cloned()
            .context("NEAR block result omitted header")?,
    )
    .context("decode NEAR block header")?;
    anyhow::ensure!(
        header.timestamp.to_string() == header.timestamp_nanosec,
        "NEAR header timestamp fields disagree"
    );
    let computed = near_head_block_hash(&header)
        .map_err(|error| anyhow!("NEAR head hash reconstruction failed: {error:?}"))?;
    anyhow::ensure!(
        bs58::encode(computed).into_string() == header.hash,
        "NEAR reconstructed head hash does not match RPC"
    );
    Ok(NearHeadBlock {
        head_height: header.height,
        head_hash: header.hash.clone(),
        head_block_merkle_root: header.block_merkle_root.clone(),
        header,
    })
}

#[derive(Debug, Deserialize)]
struct NearCodeView {
    block_hash: String,
    block_height: u64,
    code_base64: String,
    hash: String,
}

fn fetch_code_hash(
    client: &Client,
    rpc_url: &reqwest::Url,
    account_id: &str,
    head: &NearHeadBlock,
) -> Result<String> {
    let view: NearCodeView = rpc_call(
        client,
        rpc_url,
        "query",
        serde_json::json!({
            "request_type": "view_code",
            "block_id": head.head_hash,
            "account_id": account_id,
        }),
    )?;
    anyhow::ensure!(
        view.block_hash == head.head_hash && view.block_height == head.head_height,
        "NEAR code query returned a different block"
    );
    anyhow::ensure!(
        view.code_base64.len() <= MAX_NEAR_CODE_BYTES.saturating_mul(2),
        "NEAR contract code base64 exceeds bound"
    );
    let code = BASE64
        .decode(view.code_base64)
        .context("NEAR contract code was not canonical base64")?;
    anyhow::ensure!(
        !code.is_empty() && code.len() <= MAX_NEAR_CODE_BYTES,
        "NEAR contract code size is out of bounds"
    );
    let digest: [u8; 32] = Sha256::digest(code).into();
    anyhow::ensure!(
        bs58::encode(digest).into_string() == view.hash,
        "NEAR RPC code hash does not match returned code"
    );
    Ok(view.hash)
}

fn verify_code_hashes(
    client: &Client,
    rpc_url: &reqwest::Url,
    policy: &NearReceiptPolicyV1,
    head: &NearHeadBlock,
) -> Result<()> {
    anyhow::ensure!(
        fetch_code_hash(client, rpc_url, &policy.reader_account_id, head)?
            == policy.reader_code_hash,
        "NEAR reader code hash does not match policy"
    );
    anyhow::ensure!(
        fetch_code_hash(client, rpc_url, &policy.pool_id, head)? == policy.pool_code_hash,
        "NEAR pool code hash does not match policy"
    );
    Ok(())
}

fn validate_head_against_checkpoint(
    head: &NearHeadBlock,
    policy: &NearReceiptPolicyV1,
    checkpoint: &BftSourceCheckpointV1,
) -> Result<()> {
    let head_hash = decode_near_hash("NEAR head hash", &head.head_hash)?;
    let root = decode_near_hash("NEAR block merkle root", &head.head_block_merkle_root)?;
    anyhow::ensure!(
        checkpoint.source_height == head.head_height
            && checkpoint.source_block_hash == B256::from(head_hash)
            && checkpoint.source_timestamp_ms == head.header.timestamp / 1_000_000
            && checkpoint.source_state_commitment
                == policy
                    .source_state_commitment(&root)
                    .map_err(|error| anyhow!("NEAR source commitment failed: {error:?}"))?,
        "NEAR head does not match certified checkpoint"
    );
    Ok(())
}

fn validate_light_proof_foundations(
    proof: &NearLightClientProof,
    head: &NearHeadBlock,
) -> Result<()> {
    let proven_hash = near_block_hash_from_lite(&proof.block_header_lite)
        .map_err(|error| anyhow!("NEAR proven block hash failed: {error:?}"))?;
    anyhow::ensure!(
        bs58::encode(proven_hash).into_string() == proof.outcome_proof.block_hash,
        "NEAR outcome proof block hash mismatch"
    );
    let outcome_root =
        near_outcome_root(proof).map_err(|error| anyhow!("NEAR outcome root failed: {error:?}"))?;
    anyhow::ensure!(
        bs58::encode(outcome_root).into_string() == proof.block_header_lite.inner_lite.outcome_root,
        "NEAR outcome root proof mismatch"
    );
    let block_root = near_block_merkle_root_from_proof(proof)
        .map_err(|error| anyhow!("NEAR block proof failed: {error:?}"))?;
    anyhow::ensure!(
        bs58::encode(block_root).into_string() == head.head_block_merkle_root,
        "NEAR block proof does not reach certified head root"
    );
    Ok(())
}

fn decode_near_hash(label: &str, value: &str) -> Result<[u8; 32]> {
    let bytes = bs58::decode(value)
        .into_vec()
        .with_context(|| format!("{label} is not base58"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("{label} is not a 32-byte NEAR hash"))
}

fn validate_lower_hex(label: &str, value: &str, bytes: usize) -> Result<()> {
    anyhow::ensure!(
        value.len() == bytes.saturating_mul(2)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} must be canonical lowercase hex"
    );
    Ok(())
}

pub(crate) fn fuzz_external_input(data: &[u8]) {
    if data.len() > MAX_NEAR_CODE_BYTES {
        return;
    }
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = decode_near_hash("fuzz NEAR hash", text);
    }
    if let Ok(head) = serde_json::from_slice::<NearHeadBlock>(data) {
        let _ = near_head_block_hash(&head.header);
    }
    if let Ok(proof) = serde_json::from_slice::<NearLightClientProof>(data) {
        let _ = near_block_hash_from_lite(&proof.block_header_lite);
        let _ = near_outcome_root(&proof);
        let _ = near_block_merkle_root_from_proof(&proof);
    }
    #[derive(Deserialize)]
    struct CombinedProof {
        proof: NearLightClientProof,
        head: NearHeadBlock,
    }
    if let Ok(combined) = serde_json::from_slice::<CombinedProof>(data) {
        let _ = validate_light_proof_foundations(&combined.proof, &combined.head);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HISTORICAL_WITNESS: &str =
        include_str!("../../../../../benchmarks/nav-reserve-proof-historical/near-receipt-witness.json");

    #[test]
    fn historical_head_and_light_proof_reconstruct() {
        let value: Value = serde_json::from_str(HISTORICAL_WITNESS).unwrap();
        let proof: NearLightClientProof = serde_json::from_value(value["proof"].clone()).unwrap();
        let head: NearHeadBlock = serde_json::from_value(value["head"].clone()).unwrap();
        validate_light_proof_foundations(&proof, &head).unwrap();
        assert_eq!(
            bs58::encode(near_head_block_hash(&head.header).unwrap()).into_string(),
            head.head_hash
        );
    }

    #[test]
    fn near_hash_parser_is_exact() {
        let value = bs58::encode([7u8; 32]).into_string();
        assert_eq!(decode_near_hash("hash", &value).unwrap(), [7u8; 32]);
        assert!(decode_near_hash("hash", "1").is_err());
    }
}
