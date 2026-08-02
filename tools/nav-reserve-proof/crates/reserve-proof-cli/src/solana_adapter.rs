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
    solana_stake::{
        solana_stake_owner_commitment, solana_stake_reader_owner_statement_v1,
        solana_stake_reader_state_commitment_v1, verify_solana_stake_reader_proof_v1,
        SolanaStakeReaderPolicyV1, SolanaStakeReaderPositionV1, SolanaStakeReaderProofV1,
        SolanaStakeVerifyContextV1, SOLANA_STAKE_READER_ADAPTER_KIND_V1,
        SOLANA_STAKE_READER_CHECKPOINT_KIND_V1,
    },
    verify_observation_evidence, EvidenceDimensionV1, ReserveProofContextV1, SourceEvidenceV1,
    SourceManifestEntryV1, SourceObservationV1, TrustClassV1,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::evm_adapter::{
    decode_hex, load_source, read_json, rpc_call, validate_rpc_url, write_new,
};

const CLOCK_SYSVAR_ID: &str = "SysvarC1ock11111111111111111111111111111111";
const UPGRADEABLE_LOADER_ID: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const INSTRUCTION_MAGIC: &[u8; 8] = b"PFSOL001";
const SNAPSHOT_MAGIC: &[u8; 8] = b"PFSNAP01";
const SNAPSHOT_VERSION: u16 = 1;
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Subcommand)]
pub enum SolanaCommand {
    /// Emit the exact unsigned legacy transaction message for the public
    /// reader. Sign and submit it with an external Solana wallet.
    SnapshotRequest {
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        fee_payer: String,
        #[arg(long)]
        recent_blockhash: String,
        #[arg(long)]
        salt: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Fetch a finalized reader transaction, validate its public output and
    /// immutable program identity, and emit the checkpoint candidate.
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
        committee: PathBuf,
        #[arg(long)]
        transaction_signature: String,
        #[arg(long)]
        salt: String,
        #[arg(long)]
        pftl_observation_height: u64,
        #[arg(long)]
        minimum_depth: u32,
        #[arg(long)]
        rpc_url: String,
        #[arg(long)]
        prepared_output: PathBuf,
        #[arg(long)]
        checkpoint_output: PathBuf,
    },
    /// Attach the assembled checkpoint certificate and emit the exact
    /// reserve-owner statement for external signing.
    OwnerStatement {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        prepared: PathBuf,
        #[arg(long)]
        checkpoint_certificate: PathBuf,
        #[arg(long)]
        proof_output: PathBuf,
        #[arg(long)]
        owner_statement_output: PathBuf,
    },
    /// Attach the external owner signature, verify complete quantity and
    /// valuation evidence, and write the source observation.
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

pub fn run(command: SolanaCommand) -> Result<()> {
    match command {
        SolanaCommand::SnapshotRequest {
            policy,
            fee_payer,
            recent_blockhash,
            salt,
            output,
        } => snapshot_request(policy, fee_payer, recent_blockhash, salt, output),
        SolanaCommand::Prepare {
            manifest,
            context,
            source_id,
            policy,
            committee,
            transaction_signature,
            salt,
            pftl_observation_height,
            minimum_depth,
            rpc_url,
            prepared_output,
            checkpoint_output,
        } => prepare(PrepareArgs {
            manifest,
            context,
            source_id,
            policy,
            committee,
            transaction_signature,
            salt,
            pftl_observation_height,
            minimum_depth,
            rpc_url,
            prepared_output,
            checkpoint_output,
        }),
        SolanaCommand::OwnerStatement {
            manifest,
            context,
            source_id,
            prepared,
            checkpoint_certificate,
            proof_output,
            owner_statement_output,
        } => owner_statement(OwnerStatementArgs {
            manifest,
            context,
            source_id,
            prepared,
            checkpoint_certificate,
            proof_output,
            owner_statement_output,
        }),
        SolanaCommand::Collect {
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
struct SolanaSnapshotRequestV1 {
    schema: &'static str,
    fee_payer: String,
    reader_program: String,
    recent_blockhash: String,
    transaction_message_base64: String,
    unsigned_transaction_base64: String,
    instruction_data_base64: String,
    ordered_readonly_accounts: Vec<String>,
    next_required_check: &'static str,
}

fn snapshot_request(
    policy_path: PathBuf,
    fee_payer: String,
    recent_blockhash: String,
    salt: String,
    output: PathBuf,
) -> Result<()> {
    let policy: SolanaStakeReaderPolicyV1 = read_json(&policy_path)?;
    policy
        .validate()
        .map_err(|error| anyhow!("Solana reader policy is invalid: {error:?}"))?;
    let fee_payer_key = decode_base58_32("fee payer", &fee_payer)?;
    let blockhash = decode_base58_32("recent blockhash", &recent_blockhash)?;
    let salt: [u8; 32] = decode_hex("salt", &salt, 32)?
        .try_into()
        .map_err(|_| anyhow!("salt is not 32 bytes"))?;
    anyhow::ensure!(salt != [0; 32], "salt cannot be zero");
    let (message, instruction_data, ordered_accounts) =
        build_snapshot_message(&policy, fee_payer_key, blockhash, salt)?;
    let mut unsigned = vec![1];
    unsigned.extend_from_slice(&[0; 64]);
    unsigned.extend_from_slice(&message);
    let artifact = SolanaSnapshotRequestV1 {
        schema: "postfiat.reserve_solana_snapshot_request.v1",
        fee_payer,
        reader_program: policy.reader_program,
        recent_blockhash,
        transaction_message_base64: BASE64.encode(&message),
        unsigned_transaction_base64: BASE64.encode(unsigned),
        instruction_data_base64: BASE64.encode(instruction_data),
        ordered_readonly_accounts: ordered_accounts,
        next_required_check: "an external Solana wallet signs this exact transaction message, replaces the zero signature in the unsigned transaction, submits it, and retains the transaction signature",
    };
    write_new(&output, &serde_json::to_vec_pretty(&artifact)?)?;
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    Ok(())
}

fn build_snapshot_message(
    policy: &SolanaStakeReaderPolicyV1,
    fee_payer: [u8; 32],
    recent_blockhash: [u8; 32],
    salt: [u8; 32],
) -> Result<(Vec<u8>, Vec<u8>, Vec<String>)> {
    let mut account_keys = vec![
        fee_payer,
        decode_base58_32("Clock sysvar", CLOCK_SYSVAR_ID)?,
    ];
    let mut ordered_accounts = vec![CLOCK_SYSVAR_ID.to_string()];
    for position in &policy.positions {
        account_keys.push(decode_base58_32("stake account", &position.address)?);
        ordered_accounts.push(position.address.clone());
    }
    let program_index = u8::try_from(account_keys.len())?;
    account_keys.push(decode_base58_32("reader program", &policy.reader_program)?);
    anyhow::ensure!(
        account_keys.len() <= 255,
        "too many Solana message accounts"
    );
    let readonly_unsigned = u8::try_from(account_keys.len() - 1)?;
    let mut instruction_data = Vec::from(INSTRUCTION_MAGIC);
    instruction_data.extend_from_slice(&salt);
    instruction_data.extend_from_slice(&u16::try_from(policy.positions.len())?.to_le_bytes());
    let mut message = vec![1, 0, readonly_unsigned];
    write_short_vec(account_keys.len(), &mut message);
    for key in &account_keys {
        message.extend_from_slice(key);
    }
    message.extend_from_slice(&recent_blockhash);
    write_short_vec(1, &mut message);
    message.push(program_index);
    write_short_vec(ordered_accounts.len(), &mut message);
    for index in 1..=ordered_accounts.len() {
        message.push(u8::try_from(index)?);
    }
    write_short_vec(instruction_data.len(), &mut message);
    message.extend_from_slice(&instruction_data);
    Ok((message, instruction_data, ordered_accounts))
}

struct PrepareArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    committee: PathBuf,
    transaction_signature: String,
    salt: String,
    pftl_observation_height: u64,
    minimum_depth: u32,
    rpc_url: String,
    prepared_output: PathBuf,
    checkpoint_output: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedSolanaStakeV1 {
    schema: String,
    proof: SolanaStakeReaderProofV1,
}

fn prepare(args: PrepareArgs) -> Result<()> {
    anyhow::ensure!(
        !args.prepared_output.exists() && !args.checkpoint_output.exists(),
        "refusing to overwrite Solana prepared or checkpoint output"
    );
    anyhow::ensure!(args.minimum_depth > 0, "minimum depth must be nonzero");
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    validate_observation_height(&context, args.pftl_observation_height)?;
    let policy: SolanaStakeReaderPolicyV1 = read_json(&args.policy)?;
    validate_manifest_entry(&entry, &policy)?;
    anyhow::ensure!(
        args.minimum_depth >= policy.minimum_finalized_depth,
        "minimum depth is below the Solana reader policy"
    );
    let committee: BftCheckpointCommitteeV1 = read_json(&args.committee)?;
    let committee_root = committee.root().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        committee_root == policy.checkpoint_committee_root,
        "Solana checkpoint committee does not match policy"
    );
    let transaction_signature =
        decode_base58_64("transaction signature", &args.transaction_signature)?;
    let salt: [u8; 32] = decode_hex("salt", &args.salt, 32)?
        .try_into()
        .map_err(|_| anyhow!("salt is not 32 bytes"))?;
    anyhow::ensure!(salt != [0; 32], "salt cannot be zero");
    let client = rpc_client()?;
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    let transaction: TransactionResponse = rpc_call(
        &client,
        &rpc_url,
        "getTransaction",
        json!([
            args.transaction_signature,
            {"commitment":"finalized","encoding":"base64","maxSupportedTransactionVersion":0}
        ]),
    )?;
    anyhow::ensure!(
        transaction.meta.err.is_none(),
        "Solana reader transaction failed"
    );
    let wire = decode_base64_bounded("transaction", &transaction.transaction.0, 128 * 1024)?;
    let (wire_signature, message) = split_legacy_transaction(&wire)?;
    anyhow::ensure!(
        wire_signature == transaction_signature,
        "Solana RPC transaction signature differs from requested signature"
    );
    let block: SolanaBlockResponse = rpc_call(
        &client,
        &rpc_url,
        "getBlock",
        json!([
            transaction.slot,
            {"commitment":"finalized","transactionDetails":"signatures","rewards":false,"maxSupportedTransactionVersion":0}
        ]),
    )?;
    anyhow::ensure!(
        block
            .signatures
            .iter()
            .any(|value| value == &args.transaction_signature),
        "Solana transaction is absent from finalized block"
    );
    let source_hash = decode_base58_32("finalized blockhash", &block.blockhash)?;
    let observed_head: u64 = rpc_call(
        &client,
        &rpc_url,
        "getSlot",
        json!([{"commitment":"finalized"}]),
    )?;
    anyhow::ensure!(
        observed_head
            >= transaction
                .slot
                .saturating_add(u64::from(args.minimum_depth)),
        "Solana transaction has not reached the governed finalized depth"
    );
    anyhow::ensure!(
        observed_head.saturating_sub(transaction.slot) <= policy.maximum_finalized_slot_lag,
        "Solana reader transaction is stale under the governed policy"
    );
    verify_immutable_program(&client, &rpc_url, &policy)?;
    let reader_payload = extract_reader_payload(&transaction.meta.log_messages)?;
    let returned = transaction
        .meta
        .return_data
        .context("Solana reader transaction omitted return data")?;
    anyhow::ensure!(
        returned.program_id == policy.reader_program && returned.data.1 == "base64",
        "Solana return data came from the wrong program or encoding"
    );
    let return_hash = decode_base64_bounded("reader return data", &returned.data.0, 32)?;
    anyhow::ensure!(
        return_hash.len() == 32,
        "Solana reader return hash is not 32 bytes"
    );
    let parsed = parse_reader_payload(&reader_payload, &policy, salt)?;
    anyhow::ensure!(
        parsed.slot == transaction.slot,
        "Solana reader Clock slot and transaction slot differ"
    );
    let checkpoint = BftSourceCheckpointV1 {
        pftl_genesis_hash: context.pftl_genesis_hash.clone(),
        checkpoint_kind: SOLANA_STAKE_READER_CHECKPOINT_KIND_V1.to_string(),
        source_domain: policy.source_domain.clone(),
        source_height: transaction.slot,
        source_timestamp_ms: u64::try_from(
            transaction
                .block_time
                .context("Solana block time missing")?,
        )?
        .checked_mul(1_000)
        .context("Solana block timestamp overflow")?,
        source_block_hash: B256::from(source_hash),
        source_state_commitment: B256::repeat_byte(1),
        observed_source_head: observed_head,
        minimum_depth: args.minimum_depth,
        pftl_observation_height: args.pftl_observation_height,
        committee_epoch: committee.epoch,
        committee_root,
    };
    let certificate = BftSourceCheckpointCertificateV1 {
        committee,
        checkpoint,
        votes: Vec::new(),
    };
    let mut return_data_hash = [0u8; 32];
    return_data_hash.copy_from_slice(&return_hash);
    let mut proof = SolanaStakeReaderProofV1 {
        policy,
        checkpoint_certificate: certificate,
        ownership_signature: vec![0; 64],
        transaction_signature: transaction_signature.to_vec(),
        transaction_message: message,
        instruction_salt: salt,
        reader_payload,
        reader_return_data_hash: return_data_hash,
        reader_slot: parsed.slot,
        reader_epoch: parsed.epoch,
        positions: parsed.positions,
    };
    proof
        .checkpoint_certificate
        .checkpoint
        .source_state_commitment = solana_stake_reader_state_commitment_v1(&proof)
        .map_err(|error| anyhow!("Solana reader state commitment failed: {error:?}"))?;
    proof
        .checkpoint_certificate
        .checkpoint
        .canonical_bytes()
        .map_err(anyhow::Error::msg)?;
    let prepared = PreparedSolanaStakeV1 {
        schema: "postfiat.reserve_solana_prepared.v1".to_string(),
        proof,
    };
    write_new(
        &args.prepared_output,
        &serde_json::to_vec_pretty(&prepared)?,
    )?;
    write_new(
        &args.checkpoint_output,
        &serde_json::to_vec_pretty(&prepared.proof.checkpoint_certificate.checkpoint)?,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema":"postfiat.reserve_solana_prepare_report.v1",
            "prepared_output":args.prepared_output,
            "checkpoint_output":args.checkpoint_output,
            "slot":parsed.slot,
            "epoch":parsed.epoch,
            "positions":prepared.proof.positions.len(),
            "observed_source_head":observed_head,
            "next_required_check":"each checkpoint validator independently verifies the finalized block, successful reader transaction, immutable program-data hash, and canonical reader output before signing",
        }))?
    );
    Ok(())
}

struct OwnerStatementArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    prepared: PathBuf,
    checkpoint_certificate: PathBuf,
    proof_output: PathBuf,
    owner_statement_output: PathBuf,
}

fn owner_statement(args: OwnerStatementArgs) -> Result<()> {
    anyhow::ensure!(
        !args.proof_output.exists() && !args.owner_statement_output.exists(),
        "refusing to overwrite Solana proof or owner statement output"
    );
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let prepared: PreparedSolanaStakeV1 = read_json(&args.prepared)?;
    anyhow::ensure!(
        prepared.schema == "postfiat.reserve_solana_prepared.v1",
        "unsupported Solana prepared schema"
    );
    validate_manifest_entry(&entry, &prepared.proof.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    anyhow::ensure!(
        certificate.checkpoint == prepared.proof.checkpoint_certificate.checkpoint,
        "Solana certificate does not sign the prepared checkpoint"
    );
    certificate.verify().map_err(anyhow::Error::msg)?;
    let mut proof = prepared.proof;
    proof.checkpoint_certificate = certificate;
    validate_observation_height(
        &context,
        proof
            .checkpoint_certificate
            .checkpoint
            .pftl_observation_height,
    )?;
    let empty_commitment = "00".repeat(48);
    let verify_context = verification_context(
        &context,
        &entry,
        proof
            .checkpoint_certificate
            .checkpoint
            .pftl_observation_height,
        &empty_commitment,
    );
    let statement = solana_stake_reader_owner_statement_v1(&proof, &verify_context)
        .map_err(|error| anyhow!("Solana owner statement failed: {error:?}"))?;
    write_new(&args.proof_output, &serde_json::to_vec_pretty(&proof)?)?;
    write_new(&args.owner_statement_output, &statement)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema":"postfiat.reserve_solana_owner_statement.v1",
            "proof_output":args.proof_output,
            "owner_statement_output":args.owner_statement_output,
            "next_required_check":"the policy-pinned withdraw authority signs the exact statement externally; no private key enters this CLI",
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

fn collect(args: CollectArgs) -> Result<()> {
    anyhow::ensure!(
        args.total_liabilities <= args.gross_assets,
        "total liabilities exceed gross assets"
    );
    validate_lower_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let mut proof: SolanaStakeReaderProofV1 = read_json(&args.prepared_proof)?;
    validate_manifest_entry(&entry, &proof.policy)?;
    proof.ownership_signature =
        decode_base58_64("ownership signature", &args.ownership_signature)?.to_vec();
    let evidence_commitment = proof
        .evidence_commitment()
        .map_err(|error| anyhow!("Solana evidence commitment failed: {error:?}"))?;
    let observed_at = proof
        .checkpoint_certificate
        .checkpoint
        .pftl_observation_height;
    validate_observation_height(&context, observed_at)?;
    let verified = verify_solana_stake_reader_proof_v1(
        &proof,
        &verification_context(&context, &entry, observed_at, &evidence_commitment),
    )
    .map_err(|error| anyhow!("Solana reader proof failed: {error:?}"))?;
    let valuation_evidence: SourceEvidenceV1 = read_json(&args.valuation_evidence)?;
    let observation = SourceObservationV1 {
        source_id: entry.source_id.clone(),
        observed_at_block: observed_at,
        gross_assets: args.gross_assets,
        total_liabilities: args.total_liabilities,
        quantity_evidence: SourceEvidenceV1::SolanaStakeReader {
            evidence_commitment: evidence_commitment.clone(),
            proof: Box::new(proof),
        },
        valuation_evidence,
        disclosure_commitment: args.disclosure_commitment,
    };
    for dimension in [
        EvidenceDimensionV1::Quantity,
        EvidenceDimensionV1::Valuation,
    ] {
        verify_observation_evidence(&context, &entry, &observation, dimension)
            .map_err(anyhow::Error::msg)?;
    }
    write_new(&args.output, &serde_json::to_vec_pretty(&observation)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema":"postfiat.reserve_solana_collection.v1",
            "output":args.output,
            "source_id":observation.source_id,
            "slot":verified.finalized_slot,
            "epoch":verified.current_epoch,
            "total_lamports":verified.total_lamports,
            "locked_lamports":verified.locked_lamports,
            "liquid_lamports":verified.liquid_lamports,
            "quantity_trust":"cryptographic_solana_reader_bft_checkpoint",
            "valuation_trust":format!("{:?}",entry.valuation_evidence_class).to_lowercase(),
            "evidence_commitment":evidence_commitment,
        }))?
    );
    Ok(())
}

fn validate_manifest_entry(
    entry: &SourceManifestEntryV1,
    policy: &SolanaStakeReaderPolicyV1,
) -> Result<()> {
    policy
        .validate()
        .map_err(|error| anyhow!("Solana reader policy is invalid: {error:?}"))?;
    anyhow::ensure!(
        entry.adapter_kind == SOLANA_STAKE_READER_ADAPTER_KIND_V1
            && entry.adapter_schema_version == 1
            && entry.quantity_evidence_class == TrustClassV1::Cryptographic,
        "source does not use the cryptographic Solana reader adapter schema 1"
    );
    anyhow::ensure!(
        entry.source_domain == policy.source_domain
            && entry.asset_or_position_id == policy.position_set_id
            && entry.reserve_owner_commitment
                == solana_stake_owner_commitment(policy.wallet_pubkey)
            && entry.quantity_verifier_commitment
                == policy
                    .commitment()
                    .map_err(|error| anyhow!("Solana policy commitment failed: {error:?}"))?,
        "Solana manifest identity, owner, or policy commitment mismatch"
    );
    Ok(())
}

fn validate_observation_height(context: &ReserveProofContextV1, observed_at: u64) -> Result<()> {
    anyhow::ensure!(
        observed_at >= context.observation_not_before
            && observed_at <= context.observation_not_after,
        "Solana PFTL observation height is outside the context interval"
    );
    Ok(())
}

fn verification_context<'a>(
    context: &'a ReserveProofContextV1,
    entry: &'a SourceManifestEntryV1,
    observed_at: u64,
    evidence_commitment: &'a str,
) -> SolanaStakeVerifyContextV1<'a> {
    SolanaStakeVerifyContextV1 {
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

#[derive(Debug, Deserialize)]
struct TransactionResponse {
    slot: u64,
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    meta: TransactionMeta,
    transaction: (String, String),
}

#[derive(Debug, Deserialize)]
struct TransactionMeta {
    err: Option<Value>,
    #[serde(rename = "logMessages")]
    log_messages: Vec<String>,
    #[serde(rename = "returnData")]
    return_data: Option<ReturnData>,
}

#[derive(Debug, Deserialize)]
struct ReturnData {
    #[serde(rename = "programId")]
    program_id: String,
    data: (String, String),
}

#[derive(Debug, Deserialize)]
struct SolanaBlockResponse {
    blockhash: String,
    signatures: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AccountInfoResponse {
    value: SolanaAccount,
}

#[derive(Debug, Deserialize)]
struct SolanaAccount {
    executable: bool,
    owner: String,
    data: (String, String),
}

fn verify_immutable_program(
    client: &Client,
    rpc_url: &reqwest::Url,
    policy: &SolanaStakeReaderPolicyV1,
) -> Result<()> {
    let program: AccountInfoResponse = rpc_call(
        client,
        rpc_url,
        "getAccountInfo",
        json!([policy.reader_program,{"commitment":"finalized","encoding":"base64"}]),
    )?;
    anyhow::ensure!(
        program.value.executable
            && program.value.owner == UPGRADEABLE_LOADER_ID
            && program.value.data.1 == "base64",
        "Solana reader program account is not an upgradeable-loader executable"
    );
    let program_data =
        decode_base64_bounded("reader program account", &program.value.data.0, 1024)?;
    anyhow::ensure!(
        program_data.len() == 36
            && program_data[..4] == 2u32.to_le_bytes()
            && program_data[4..36]
                == decode_base58_32("program-data address", &policy.reader_program_data)?,
        "Solana reader program does not point to the policy program-data account"
    );
    let code: AccountInfoResponse = rpc_call(
        client,
        rpc_url,
        "getAccountInfo",
        json!([policy.reader_program_data,{"commitment":"finalized","encoding":"base64"}]),
    )?;
    anyhow::ensure!(
        !code.value.executable
            && code.value.owner == UPGRADEABLE_LOADER_ID
            && code.value.data.1 == "base64",
        "Solana reader program-data account is invalid"
    );
    let code_data =
        decode_base64_bounded("reader program data", &code.value.data.0, 2 * 1024 * 1024)?;
    verify_immutable_program_data(&code_data)?;
    let digest: [u8; 32] = Sha256::digest(&code_data).into();
    anyhow::ensure!(
        digest == policy.reader_program_data_hash,
        "Solana reader program-data hash does not match policy"
    );
    Ok(())
}

fn verify_immutable_program_data(data: &[u8]) -> Result<()> {
    // UpgradeableLoaderState::ProgramData is encoded as the four-byte enum
    // discriminator, an eight-byte deployment slot, and an Option<Pubkey>
    // upgrade authority. Solana reserves the maximum 45-byte metadata prefix
    // before executable bytes. Bytes after the None tag can retain the former
    // authority when the state is rewritten, so only the tag is authoritative.
    anyhow::ensure!(
        data.len() > 45 && data[..4] == 3u32.to_le_bytes() && data[12] == 0,
        "Solana reader program is not immutable"
    );
    Ok(())
}

fn extract_reader_payload(logs: &[String]) -> Result<Vec<u8>> {
    let mut payload = None;
    for log in logs {
        let Some(encoded) = log.strip_prefix("Program data: ") else {
            continue;
        };
        let decoded = decode_base64_bounded("Solana reader log", encoded, 16 * 1024)?;
        if decoded.starts_with(SNAPSHOT_MAGIC) {
            anyhow::ensure!(
                payload.replace(decoded).is_none(),
                "duplicate Solana reader payload"
            );
        }
    }
    payload.context("Solana reader payload log is missing")
}

struct ParsedReaderPayload {
    slot: u64,
    epoch: u64,
    positions: Vec<SolanaStakeReaderPositionV1>,
}

fn parse_reader_payload(
    payload: &[u8],
    policy: &SolanaStakeReaderPolicyV1,
    expected_salt: [u8; 32],
) -> Result<ParsedReaderPayload> {
    let mut reader = SliceReader::new(payload);
    anyhow::ensure!(
        reader.bytes(8)? == SNAPSHOT_MAGIC,
        "bad Solana snapshot magic"
    );
    anyhow::ensure!(
        reader.u16()? == SNAPSHOT_VERSION,
        "bad Solana snapshot version"
    );
    let slot = reader.u64()?;
    let epoch = reader.u64()?;
    anyhow::ensure!(
        reader.array_32()? == expected_salt,
        "Solana snapshot salt mismatch"
    );
    let count = usize::from(reader.u16()?);
    anyhow::ensure!(
        count == policy.positions.len(),
        "Solana snapshot position count mismatch"
    );
    let mut positions = Vec::with_capacity(count);
    for position in &policy.positions {
        let address = reader.array_32()?;
        let lamports = reader.u64()?;
        let owner = reader.array_32()?;
        let data_hash = reader.array_32()?;
        let stake_authority = reader.array_32()?;
        let withdraw_authority = reader.array_32()?;
        let vote_account = reader.array_32()?;
        let delegated_lamports = reader.u64()?;
        let activation_epoch = reader.u64()?;
        let deactivation_epoch = reader.u64()?;
        anyhow::ensure!(
            address == decode_base58_32("stake position", &position.address)?
                && owner == decode_base58_32("stake program", &policy.stake_program)?
                && stake_authority == policy.stake_authority
                && withdraw_authority == policy.withdraw_authority
                && vote_account == position.vote_account
                && data_hash != [0; 32]
                && delegated_lamports <= lamports,
            "Solana reader payload violates position policy"
        );
        positions.push(SolanaStakeReaderPositionV1 {
            index: position.index,
            address: position.address.clone(),
            lamports,
            owner_program: policy.stake_program.clone(),
            data_hash,
            stake_authority,
            withdraw_authority,
            vote_account,
            delegated_lamports,
            activation_epoch,
            deactivation_epoch,
        });
    }
    reader.finish()?;
    Ok(ParsedReaderPayload {
        slot,
        epoch,
        positions,
    })
}

fn split_legacy_transaction(wire: &[u8]) -> Result<([u8; 64], Vec<u8>)> {
    let mut reader = SliceReader::new(wire);
    anyhow::ensure!(
        reader.short_vec()? == 1,
        "Solana reader transaction must have one signature"
    );
    let signature: [u8; 64] = reader
        .bytes(64)?
        .try_into()
        .context("Solana transaction signature")?;
    let message = reader.remaining().to_vec();
    anyhow::ensure!(
        !message.is_empty() && message[0] & 0x80 == 0,
        "Solana transaction is not legacy"
    );
    Ok((signature, message))
}

struct SliceReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SliceReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .context("Solana payload offset overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .context("Solana payload ended early")?;
        self.offset = end;
        Ok(value)
    }
    fn array_32(&mut self) -> Result<[u8; 32]> {
        self.bytes(32)?.try_into().context("Solana 32-byte field")
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into()?))
    }
    fn short_vec(&mut self) -> Result<usize> {
        let start = self.offset;
        let mut value = 0usize;
        let mut shift = 0u32;
        loop {
            let byte = *self.bytes(1)?.first().context("Solana shortvec")?;
            anyhow::ensure!(shift < usize::BITS, "Solana shortvec overflow");
            value |= usize::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                let mut canonical = Vec::new();
                write_short_vec(value, &mut canonical);
                anyhow::ensure!(
                    self.bytes[start..self.offset] == canonical,
                    "non-canonical Solana shortvec"
                );
                return Ok(value);
            }
            shift += 7;
        }
    }
    fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.offset..]
    }
    fn finish(&self) -> Result<()> {
        anyhow::ensure!(
            self.offset == self.bytes.len(),
            "Solana payload has trailing bytes"
        );
        Ok(())
    }
}

fn write_short_vec(mut value: usize, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn rpc_client() -> Result<Client> {
    Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()
        .context("build Solana RPC client")
}

fn decode_base58_32(label: &str, value: &str) -> Result<[u8; 32]> {
    let bytes = bs58::decode(value)
        .into_vec()
        .with_context(|| format!("{label} is not base58"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("{label} is not 32 bytes"))
}

fn decode_base58_64(label: &str, value: &str) -> Result<[u8; 64]> {
    let bytes = bs58::decode(value)
        .into_vec()
        .with_context(|| format!("{label} is not base58"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("{label} is not 64 bytes"))
}

fn decode_base64_bounded(label: &str, value: &str, maximum: usize) -> Result<Vec<u8>> {
    anyhow::ensure!(
        value.len() <= maximum.saturating_mul(2),
        "{label} base64 exceeds bound"
    );
    let bytes = BASE64
        .decode(value)
        .with_context(|| format!("decode {label} base64"))?;
    anyhow::ensure!(bytes.len() <= maximum, "{label} exceeds bound");
    Ok(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SolanaStakeReaderPolicyV1 {
        SolanaStakeReaderPolicyV1 {
            source_domain: "solana:mainnet".to_string(),
            position_set_id: "solana-stake-set:test".to_string(),
            stake_program: "Stake11111111111111111111111111111111111111".to_string(),
            reader_program: bs58::encode([4; 32]).into_string(),
            reader_program_data: bs58::encode([5; 32]).into_string(),
            reader_program_data_hash: [6; 32],
            wallet: bs58::encode([7; 32]).into_string(),
            wallet_pubkey: [7; 32],
            stake_authority: [7; 32],
            withdraw_authority: [7; 32],
            positions: vec![
                reserve_proof_types::solana_stake::SolanaStakePositionPolicyV1 {
                    index: 0,
                    address: bs58::encode([8; 32]).into_string(),
                    vote_account: [9; 32],
                },
            ],
            checkpoint_committee_root: "11".repeat(48),
            minimum_finalized_depth: 32,
            maximum_finalized_slot_lag: 512,
        }
    }

    #[test]
    fn snapshot_message_is_canonical_and_exact() {
        let (message, data, accounts) =
            build_snapshot_message(&policy(), [7; 32], [3; 32], [2; 32]).unwrap();
        assert_eq!(&data[..8], INSTRUCTION_MAGIC);
        assert_eq!(accounts.len(), 2);
        assert_eq!(message[0], 1);
        assert!(message.ends_with(&data));
    }

    #[test]
    fn payload_parser_rejects_omission_and_trailing_data() {
        let mut payload = Vec::from(SNAPSHOT_MAGIC);
        payload.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
        payload.extend_from_slice(&77u64.to_le_bytes());
        payload.extend_from_slice(&9u64.to_le_bytes());
        payload.extend_from_slice(&[2; 32]);
        payload.extend_from_slice(&0u16.to_le_bytes());
        assert!(parse_reader_payload(&payload, &policy(), [2; 32]).is_err());
        payload.push(0);
        assert!(parse_reader_payload(&payload, &policy(), [2; 32]).is_err());
    }

    #[test]
    fn shortvec_rejects_noncanonical_encoding() {
        let mut reader = SliceReader::new(&[0x80, 0x00]);
        assert!(reader.short_vec().is_err());
    }

    #[test]
    fn program_data_requires_absent_upgrade_authority_and_executable_bytes() {
        let mut data = vec![0u8; 46];
        data[..4].copy_from_slice(&3u32.to_le_bytes());
        assert!(verify_immutable_program_data(&data).is_ok());
        data[12] = 1;
        assert!(verify_immutable_program_data(&data).is_err());
        data[12] = 0;
        data[20] = 1;
        assert!(verify_immutable_program_data(&data).is_ok());
        assert!(verify_immutable_program_data(&data[..45]).is_err());
    }
}
