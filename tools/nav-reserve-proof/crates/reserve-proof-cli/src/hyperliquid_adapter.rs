use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use alloy_primitives::{keccak256, Address, B256};
use alloy_rlp::{Decodable, Encodable, Header};
use alloy_trie::{
    nodes::{BranchNode, RlpNode, TrieNode, CHILD_INDEX_RANGE},
    proof::ProofRetainer,
    root::adjust_index_for_rlp,
    HashBuilder, Nibbles,
};
use anyhow::{anyhow, bail, Context, Result};
use clap::Subcommand;
use reqwest::{blocking::Client, redirect::Policy};
use reserve_proof_types::{
    bft_checkpoint::{
        BftCheckpointCommitteeV1, BftSourceCheckpointCertificateV1, BftSourceCheckpointV1,
    },
    hyperliquid_receipt::{
        hl_receipt_event_topic0, hyperliquid_owner_authorization_statement_for_policy_v1,
        hyperliquid_owner_commitment, hyperliquid_source_state_commitment_v1,
        verify_hyperliquid_receipt_proof_v1, HyperliquidReceiptPolicyV1, HyperliquidReceiptProofV1,
        HyperliquidReceiptVerifyContextV1, HyperliquidSpotTokenPolicyV1,
        HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1, MAX_HYPERLIQUID_LOG_TOPICS,
        MAX_HYPERLIQUID_RECEIPT_BYTES, MAX_HYPERLIQUID_RECEIPT_LOGS,
    },
    verify_observation_evidence, EvidenceDimensionV1, ReserveProofContextV1, SourceEvidenceV1,
    SourceManifestEntryV1, SourceObservationV1, TrustClassV1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::evm_adapter::{
    decode_hex, load_source, parse_address, parse_b256, parse_u64_quantity, read_json, rpc_call,
    validate_rpc_url, write_new,
};

const HYPEREVM_HEADER_CHECKPOINT_KIND_V1: &str = "hyperevm-header";
const SNAPSHOT_FUNCTION_SIGNATURE: &[u8] =
    b"snapshot(address,uint32[],(uint64,uint8,uint32,uint8)[],bytes32)";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BLOCK_RECEIPTS: usize = 8_192;
const MAX_HYPEREVM_FUZZ_INPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub enum HyperliquidCommand {
    /// Emit an unsigned HyperEVM transaction request for the public reader.
    SnapshotRequest {
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        salt: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Query HyperEVM and emit the exact header checkpoint validators must
    /// independently reproduce before signing.
    CheckpointCandidate {
        #[arg(long)]
        pftl_genesis_hash: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        source_height: u64,
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
    },
    /// Emit the exact EIP-191 statement authorizing the governed reader,
    /// policy, and source checkpoint.
    OwnerAuthorization {
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
        owner: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Reconstruct the finalized block header, every receipt, and the target
    /// receipt-trie proof, then emit a fully verified source observation.
    Collect {
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
        owner: String,
        #[arg(long)]
        ownership_signature: String,
        #[arg(long)]
        snapshot_tx_hash: String,
        #[arg(long)]
        salt: String,
        #[arg(long)]
        gross_assets: u64,
        #[arg(long, default_value_t = 0)]
        total_liabilities: u64,
        #[arg(long)]
        disclosure_commitment: String,
        #[arg(long)]
        rpc_url: String,
        #[arg(long)]
        output: PathBuf,
    },
}

pub fn run(command: HyperliquidCommand) -> Result<()> {
    match command {
        HyperliquidCommand::SnapshotRequest {
            policy,
            owner,
            salt,
            output,
        } => snapshot_request(policy, owner, salt, output),
        HyperliquidCommand::CheckpointCandidate {
            pftl_genesis_hash,
            policy,
            source_height,
            minimum_depth,
            pftl_observation_height,
            committee,
            rpc_url,
            output,
        } => checkpoint_candidate(CheckpointCandidateArgs {
            pftl_genesis_hash,
            policy,
            source_height,
            minimum_depth,
            pftl_observation_height,
            committee,
            rpc_url,
            output,
        }),
        HyperliquidCommand::OwnerAuthorization {
            manifest,
            context,
            source_id,
            policy,
            checkpoint_certificate,
            owner,
            output,
        } => owner_authorization(OwnerAuthorizationArgs {
            manifest,
            context,
            source_id,
            policy,
            checkpoint_certificate,
            owner,
            output,
        }),
        HyperliquidCommand::Collect {
            manifest,
            context,
            source_id,
            policy,
            checkpoint_certificate,
            owner,
            ownership_signature,
            snapshot_tx_hash,
            salt,
            gross_assets,
            total_liabilities,
            disclosure_commitment,
            rpc_url,
            output,
        } => collect(CollectArgs {
            manifest,
            context,
            source_id,
            policy,
            checkpoint_certificate,
            owner,
            ownership_signature,
            snapshot_tx_hash,
            salt,
            gross_assets,
            total_liabilities,
            disclosure_commitment,
            rpc_url,
            output,
        }),
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct SnapshotRequestV1 {
    schema: &'static str,
    chain_id: u64,
    to: Address,
    value: &'static str,
    data: String,
    owner: Address,
    salt: B256,
    required_perps: Vec<u32>,
    spot_tokens: Vec<HyperliquidSpotTokenPolicyV1>,
    reader_code_hash: B256,
}

fn snapshot_request(
    policy_path: PathBuf,
    owner: String,
    salt: String,
    output: PathBuf,
) -> Result<()> {
    let policy: HyperliquidReceiptPolicyV1 = read_json(&policy_path)?;
    policy
        .validate()
        .map_err(|error| anyhow!("Hyperliquid policy is invalid: {error:?}"))?;
    let owner = parse_address("owner", &owner)?;
    anyhow::ensure!(owner != Address::ZERO, "owner cannot be zero");
    let salt = parse_b256("salt", &salt)?;
    anyhow::ensure!(salt != B256::ZERO, "salt cannot be zero");
    let calldata = snapshot_calldata(&policy, owner, salt)?;
    let request = SnapshotRequestV1 {
        schema: "postfiat.reserve_hyperliquid_snapshot_request.v1",
        chain_id: policy.hyperevm_chain_id,
        to: policy.reader_contract,
        value: "0x0",
        data: format!("0x{}", hex::encode(calldata)),
        owner,
        salt,
        required_perps: policy.required_perps,
        spot_tokens: policy.allowed_spot_tokens,
        reader_code_hash: policy.reader_code_hash,
    };
    write_new(&output, &serde_json::to_vec_pretty(&request)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_hyperliquid_snapshot_request_report.v1",
            "output": output,
            "chain_id": request.chain_id,
            "reader_contract": request.to,
            "owner": request.owner,
            "perp_count": request.required_perps.len(),
            "spot_count": request.spot_tokens.len(),
            "next_required_check": "sign and submit this exact zero-value transaction with an external wallet, then pass its hash to collect",
        }))?
    );
    Ok(())
}

struct CheckpointCandidateArgs {
    pftl_genesis_hash: String,
    policy: PathBuf,
    source_height: u64,
    minimum_depth: u32,
    pftl_observation_height: u64,
    committee: PathBuf,
    rpc_url: String,
    output: PathBuf,
}

fn checkpoint_candidate(args: CheckpointCandidateArgs) -> Result<()> {
    validate_lower_hex("pftl_genesis_hash", &args.pftl_genesis_hash, 48)?;
    anyhow::ensure!(
        args.source_height > 0 && args.minimum_depth > 0 && args.pftl_observation_height > 0,
        "checkpoint heights and minimum depth must be nonzero"
    );
    let policy: HyperliquidReceiptPolicyV1 = read_json(&args.policy)?;
    policy
        .validate()
        .map_err(|error| anyhow!("Hyperliquid policy is invalid: {error:?}"))?;
    let committee: BftCheckpointCommitteeV1 = read_json(&args.committee)?;
    let committee_root = committee.root().map_err(anyhow::Error::msg)?;
    policy
        .commitment(&committee_root)
        .map_err(|error| anyhow!("Hyperliquid policy commitment failed: {error:?}"))?;
    let client = rpc_client()?;
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    validate_chain_id(&client, &rpc_url, &policy)?;
    let observed_source_head = rpc_head(&client, &rpc_url)?;
    let required_head = args
        .source_height
        .checked_add(u64::from(args.minimum_depth))
        .context("HyperEVM checkpoint depth overflows")?;
    anyhow::ensure!(
        observed_source_head >= required_head,
        "HyperEVM source block has not reached the required depth"
    );
    let block_tag = format!("0x{:x}", args.source_height);
    let block: Value = rpc_call(
        &client,
        &rpc_url,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    let encoded = validate_and_encode_block(&block, args.source_height)?;
    let reader_code_hash = fetch_reader_code_hash(&client, &rpc_url, &policy, &block_tag)?;
    anyhow::ensure!(
        reader_code_hash == policy.reader_code_hash,
        "HyperEVM reader code hash does not match policy"
    );
    let checkpoint = BftSourceCheckpointV1 {
        pftl_genesis_hash: args.pftl_genesis_hash,
        checkpoint_kind: HYPEREVM_HEADER_CHECKPOINT_KIND_V1.to_string(),
        source_domain: policy.source_domain.clone(),
        source_height: args.source_height,
        source_timestamp_ms: encoded.timestamp_ms,
        source_block_hash: encoded.block_hash,
        source_state_commitment: hyperliquid_source_state_commitment_v1(
            encoded.receipts_root,
            policy.reader_contract,
            policy.reader_code_hash,
        ),
        observed_source_head,
        minimum_depth: args.minimum_depth,
        pftl_observation_height: args.pftl_observation_height,
        committee_epoch: committee.epoch,
        committee_root,
    };
    checkpoint.canonical_bytes().map_err(anyhow::Error::msg)?;
    write_new(&args.output, &serde_json::to_vec_pretty(&checkpoint)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_hyperliquid_checkpoint_candidate.v1",
            "output": args.output,
            "source_height": checkpoint.source_height,
            "source_block_hash": checkpoint.source_block_hash,
            "receipts_root": encoded.receipts_root,
            "reader_code_hash": policy.reader_code_hash,
            "source_state_commitment": checkpoint.source_state_commitment,
            "observed_source_head": checkpoint.observed_source_head,
            "minimum_depth": checkpoint.minimum_depth,
            "next_required_check": "each validator independently checks the header, reader code, and depth before signing",
        }))?
    );
    Ok(())
}

struct OwnerAuthorizationArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    checkpoint_certificate: PathBuf,
    owner: String,
    output: PathBuf,
}

fn owner_authorization(args: OwnerAuthorizationArgs) -> Result<()> {
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: HyperliquidReceiptPolicyV1 = read_json(&args.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    let owner = parse_address("owner", &args.owner)?;
    validate_manifest_entry(
        &entry,
        &policy,
        &certificate,
        owner,
        &context.pftl_genesis_hash,
    )?;
    let empty_evidence_commitment = "00".repeat(48);
    let verify_context = verification_context(
        &context,
        &entry,
        certificate.checkpoint.pftl_observation_height,
        0,
        0,
        &empty_evidence_commitment,
    );
    let statement = hyperliquid_owner_authorization_statement_for_policy_v1(
        &policy,
        &certificate,
        owner,
        &verify_context,
    )
    .map_err(|error| anyhow!("Hyperliquid owner statement failed: {error:?}"))?;
    write_new(&args.output, &statement)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_hyperliquid_owner_authorization.v1",
            "output": args.output,
            "statement_bytes": statement.len(),
            "statement_hex": hex::encode(statement),
        }))?
    );
    Ok(())
}

struct CollectArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    checkpoint_certificate: PathBuf,
    owner: String,
    ownership_signature: String,
    snapshot_tx_hash: String,
    salt: String,
    gross_assets: u64,
    total_liabilities: u64,
    disclosure_commitment: String,
    rpc_url: String,
    output: PathBuf,
}

fn collect(args: CollectArgs) -> Result<()> {
    anyhow::ensure!(
        args.total_liabilities <= args.gross_assets,
        "total liabilities exceed gross assets"
    );
    validate_lower_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: HyperliquidReceiptPolicyV1 = read_json(&args.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    let owner = parse_address("owner", &args.owner)?;
    validate_manifest_entry(
        &entry,
        &policy,
        &certificate,
        owner,
        &context.pftl_genesis_hash,
    )?;
    let checkpoint = &certificate.checkpoint;
    let source_height = checkpoint.source_height;
    let observed_at_pftl_height = checkpoint.pftl_observation_height;
    anyhow::ensure!(
        observed_at_pftl_height >= context.observation_not_before
            && observed_at_pftl_height <= context.observation_not_after,
        "Hyperliquid checkpoint PFTL height is outside the observation interval"
    );
    let ownership_signature = decode_hex("ownership_signature", &args.ownership_signature, 65)?;
    let snapshot_tx_hash = parse_b256("snapshot_tx_hash", &args.snapshot_tx_hash)?;
    let salt = parse_b256("salt", &args.salt)?;
    anyhow::ensure!(salt != B256::ZERO, "salt cannot be zero");

    let client = rpc_client()?;
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    validate_chain_id(&client, &rpc_url, &policy)?;
    let block_tag = format!("0x{source_height:x}");
    let block: Value = rpc_call(
        &client,
        &rpc_url,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    let encoded_block = validate_and_encode_block(&block, source_height)?;
    anyhow::ensure!(
        encoded_block.block_hash == checkpoint.source_block_hash
            && encoded_block.timestamp_ms == checkpoint.source_timestamp_ms,
        "HyperEVM block does not match the certified checkpoint"
    );
    let reader_code_hash = fetch_reader_code_hash(&client, &rpc_url, &policy, &block_tag)?;
    anyhow::ensure!(
        reader_code_hash == policy.reader_code_hash,
        "HyperEVM reader code hash does not match policy"
    );
    anyhow::ensure!(
        checkpoint.source_state_commitment
            == hyperliquid_source_state_commitment_v1(
                encoded_block.receipts_root,
                policy.reader_contract,
                policy.reader_code_hash,
            ),
        "HyperEVM receipts root or reader identity does not match the certified checkpoint"
    );
    anyhow::ensure!(
        rpc_head(&client, &rpc_url)? >= checkpoint.observed_source_head,
        "HyperEVM RPC head is behind the certified observation head"
    );

    let receipts: Vec<RpcReceipt> = rpc_call(
        &client,
        &rpc_url,
        "eth_getBlockReceipts",
        serde_json::json!([block_tag]),
    )?;
    let receipts = validate_and_order_receipts(
        receipts,
        checkpoint.source_height,
        checkpoint.source_block_hash,
    )?;
    let target_index = select_target_receipt(&receipts, policy.reader_contract, snapshot_tx_hash)?;
    let encoded_receipts = receipts
        .iter()
        .map(encode_receipt)
        .collect::<Result<Vec<_>>>()?;
    let (reconstructed_root, receipt_proof_nodes) =
        build_receipts_trie_with_proof(&encoded_receipts, target_index)?;
    anyhow::ensure!(
        reconstructed_root == encoded_block.receipts_root,
        "reconstructed receipts root does not match the certified header"
    );

    let proof = HyperliquidReceiptProofV1 {
        policy,
        checkpoint_certificate: certificate,
        owner,
        ownership_signature,
        block_header_rlp: encoded_block.header_rlp,
        receipt_index: u64::try_from(target_index).context("receipt index exceeds u64")?,
        receipt_rlp: encoded_receipts[target_index].clone(),
        receipt_proof_nodes: receipt_proof_nodes
            .into_iter()
            .map(|node| node.to_vec())
            .collect(),
        salt,
    };
    let evidence_commitment = proof
        .commitment()
        .map_err(|error| anyhow!("Hyperliquid evidence commitment failed: {error:?}"))?;
    let verify_context = verification_context(
        &context,
        &entry,
        observed_at_pftl_height,
        args.gross_assets,
        args.total_liabilities,
        &evidence_commitment,
    );
    let verified = verify_hyperliquid_receipt_proof_v1(&proof, &verify_context)
        .map_err(|error| anyhow!("Hyperliquid receipt proof failed: {error:?}"))?;
    let evidence = SourceEvidenceV1::HyperliquidReceipt {
        evidence_commitment: evidence_commitment.clone(),
        proof: Box::new(proof),
    };
    let observation = SourceObservationV1 {
        source_id: entry.source_id.clone(),
        observed_at_block: observed_at_pftl_height,
        gross_assets: verified.gross_assets_usd_e8,
        total_liabilities: verified.total_liabilities_usd_e8,
        quantity_evidence: evidence.clone(),
        valuation_evidence: evidence,
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
            "schema": "postfiat.reserve_hyperliquid_collection.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "source_height": source_height,
            "pftl_observation_height": observation.observed_at_block,
            "snapshot_tx_hash": snapshot_tx_hash,
            "receipt_index": target_index,
            "gross_assets_usd_e8": verified.gross_assets_usd_e8,
            "total_liabilities_usd_e8": verified.total_liabilities_usd_e8,
            "perp_count": verified.payload.perps.len(),
            "spot_count": verified.payload.spots.len(),
            "evidence_commitment": evidence_commitment,
            "quantity_trust": "cryptographic_bft_checkpoint_receipt",
            "valuation_trust": "cryptographic_hypercore_precompile_receipt",
            "next_required_check": "postfiat-reserve-proof observe validates this source again in the complete manifest",
        }))?
    );
    Ok(())
}

fn validate_manifest_entry(
    entry: &SourceManifestEntryV1,
    policy: &HyperliquidReceiptPolicyV1,
    certificate: &BftSourceCheckpointCertificateV1,
    owner: Address,
    pftl_genesis_hash: &str,
) -> Result<()> {
    policy
        .validate()
        .map_err(|error| anyhow!("Hyperliquid policy is invalid: {error:?}"))?;
    certificate.verify().map_err(anyhow::Error::msg)?;
    let checkpoint = &certificate.checkpoint;
    anyhow::ensure!(
        checkpoint.pftl_genesis_hash == pftl_genesis_hash
            && checkpoint.checkpoint_kind == HYPEREVM_HEADER_CHECKPOINT_KIND_V1
            && checkpoint.source_domain == policy.source_domain,
        "Hyperliquid checkpoint does not match genesis, kind, or policy"
    );
    anyhow::ensure!(
        policy.source_domain == format!("eip155:{}", policy.hyperevm_chain_id),
        "Hyperliquid policy source domain is not canonical"
    );
    let committee_root = certificate.committee.root().map_err(anyhow::Error::msg)?;
    let policy_commitment = policy
        .commitment(&committee_root)
        .map_err(|error| anyhow!("Hyperliquid policy commitment failed: {error:?}"))?;
    anyhow::ensure!(
        entry.adapter_kind == HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1
            && entry.adapter_schema_version == 1,
        "source does not use {HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1} schema 1"
    );
    anyhow::ensure!(
        entry.quantity_evidence_class == TrustClassV1::Cryptographic
            && entry.valuation_evidence_class == TrustClassV1::Cryptographic,
        "Hyperliquid quantity and valuation must both be cryptographic"
    );
    anyhow::ensure!(
        entry.source_domain == policy.source_domain
            && entry.asset_or_position_id == format!("hyperliquid:account:{owner:#x}")
            && entry.reserve_owner_commitment == hyperliquid_owner_commitment(owner)
            && entry.quantity_verifier_commitment == policy_commitment
            && entry.valuation_verifier_commitment == policy_commitment,
        "Hyperliquid manifest identity, owner, or policy commitment mismatch"
    );
    Ok(())
}

fn verification_context<'a>(
    context: &'a ReserveProofContextV1,
    entry: &'a SourceManifestEntryV1,
    observed_at_pftl_height: u64,
    expected_gross_assets: u64,
    expected_total_liabilities: u64,
    expected_evidence_commitment: &'a str,
) -> HyperliquidReceiptVerifyContextV1<'a> {
    HyperliquidReceiptVerifyContextV1 {
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
        valuation_verifier_commitment: &entry.valuation_verifier_commitment,
        observed_at_pftl_height,
        expected_gross_assets,
        expected_total_liabilities,
        expected_evidence_commitment,
    }
}

fn rpc_client() -> Result<Client> {
    Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()
        .context("build HyperEVM RPC client")
}

fn validate_chain_id(
    client: &Client,
    rpc_url: &reqwest::Url,
    policy: &HyperliquidReceiptPolicyV1,
) -> Result<()> {
    let chain_id: String = rpc_call(client, rpc_url, "eth_chainId", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_rpc_quantity("eth_chainId", &chain_id)? == policy.hyperevm_chain_id,
        "HyperEVM RPC chain ID does not match policy"
    );
    Ok(())
}

fn rpc_head(client: &Client, rpc_url: &reqwest::Url) -> Result<u64> {
    let head: String = rpc_call(client, rpc_url, "eth_blockNumber", serde_json::json!([]))?;
    parse_rpc_quantity("eth_blockNumber", &head)
}

fn fetch_reader_code_hash(
    client: &Client,
    rpc_url: &reqwest::Url,
    policy: &HyperliquidReceiptPolicyV1,
    block_tag: &str,
) -> Result<B256> {
    let code: String = rpc_call(
        client,
        rpc_url,
        "eth_getCode",
        serde_json::json!([format!("{:#x}", policy.reader_contract), block_tag]),
    )?;
    let code = decode_data("eth_getCode", &code, 512 * 1024)?;
    anyhow::ensure!(
        !code.is_empty(),
        "HyperEVM reader has no code at checkpoint"
    );
    Ok(keccak256(code))
}

struct EncodedBlock {
    header_rlp: Vec<u8>,
    block_hash: B256,
    receipts_root: B256,
    timestamp_ms: u64,
}

fn validate_and_encode_block(block: &Value, expected_height: u64) -> Result<EncodedBlock> {
    anyhow::ensure!(
        block.is_object(),
        "eth_getBlockByNumber returned null or non-object"
    );
    anyhow::ensure!(
        parse_rpc_quantity("block.number", required_str(block, "number")?)? == expected_height,
        "HyperEVM RPC substituted a different block"
    );
    let header_rlp = encode_block_header(block)?;
    let block_hash = parse_b256("block.hash", required_str(block, "hash")?)?;
    anyhow::ensure!(
        keccak256(&header_rlp) == block_hash,
        "recomputed HyperEVM block hash does not match RPC block hash"
    );
    let receipts_root = parse_b256("block.receiptsRoot", required_str(block, "receiptsRoot")?)?;
    anyhow::ensure!(
        receipts_root != B256::ZERO,
        "HyperEVM receipts root is zero"
    );
    let timestamp_ms = parse_rpc_quantity("block.timestamp", required_str(block, "timestamp")?)?
        .checked_mul(1_000)
        .context("HyperEVM timestamp milliseconds overflow")?;
    Ok(EncodedBlock {
        header_rlp,
        block_hash,
        receipts_root,
        timestamp_ms,
    })
}

fn encode_block_header(block: &Value) -> Result<Vec<u8>> {
    let mut fields = Vec::new();
    for (name, kind) in [
        ("parentHash", HeaderFieldKind::Fixed(32)),
        ("sha3Uncles", HeaderFieldKind::Fixed(32)),
        ("miner", HeaderFieldKind::Fixed(20)),
        ("stateRoot", HeaderFieldKind::Fixed(32)),
        ("transactionsRoot", HeaderFieldKind::Fixed(32)),
        ("receiptsRoot", HeaderFieldKind::Fixed(32)),
        ("logsBloom", HeaderFieldKind::Fixed(256)),
        ("difficulty", HeaderFieldKind::Quantity),
        ("number", HeaderFieldKind::Quantity),
        ("gasLimit", HeaderFieldKind::Quantity),
        ("gasUsed", HeaderFieldKind::Quantity),
        ("timestamp", HeaderFieldKind::Quantity),
        ("extraData", HeaderFieldKind::Data),
        ("mixHash", HeaderFieldKind::Fixed(32)),
        ("nonce", HeaderFieldKind::Fixed(8)),
    ] {
        fields.push(encode_header_field(block, name, kind)?);
    }
    if has_non_null(block, "baseFeePerGas") {
        fields.push(encode_header_field(
            block,
            "baseFeePerGas",
            HeaderFieldKind::Quantity,
        )?);
    }
    if has_non_null(block, "withdrawalsRoot") {
        fields.push(encode_header_field(
            block,
            "withdrawalsRoot",
            HeaderFieldKind::Fixed(32),
        )?);
    }
    let has_blob_gas = has_non_null(block, "blobGasUsed");
    let has_excess_blob_gas = has_non_null(block, "excessBlobGas");
    anyhow::ensure!(
        has_blob_gas == has_excess_blob_gas,
        "HyperEVM block has only one EIP-4844 gas field"
    );
    if has_blob_gas {
        fields.push(encode_header_field(
            block,
            "blobGasUsed",
            HeaderFieldKind::Quantity,
        )?);
        fields.push(encode_header_field(
            block,
            "excessBlobGas",
            HeaderFieldKind::Quantity,
        )?);
    }
    if has_non_null(block, "parentBeaconBlockRoot") {
        fields.push(encode_header_field(
            block,
            "parentBeaconBlockRoot",
            HeaderFieldKind::Fixed(32),
        )?);
    }
    if has_non_null(block, "requestsHash") {
        fields.push(encode_header_field(
            block,
            "requestsHash",
            HeaderFieldKind::Fixed(32),
        )?);
    }
    Ok(rlp_list(fields))
}

#[derive(Clone, Copy)]
enum HeaderFieldKind {
    Fixed(usize),
    Quantity,
    Data,
}

fn encode_header_field(block: &Value, name: &str, kind: HeaderFieldKind) -> Result<Vec<u8>> {
    let value = required_str(block, name)?;
    match kind {
        HeaderFieldKind::Fixed(bytes) => rlp_fixed_bytes(name, value, bytes),
        HeaderFieldKind::Quantity => rlp_quantity(name, value),
        HeaderFieldKind::Data => Ok(rlp_bytes(&decode_data(name, value, 128 * 1024)?)),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcReceipt {
    #[serde(rename = "type")]
    tx_type: Option<String>,
    status: Option<String>,
    root: Option<String>,
    cumulative_gas_used: String,
    logs_bloom: String,
    logs: Vec<RpcLog>,
    transaction_hash: String,
    transaction_index: String,
    block_hash: String,
    block_number: String,
    #[serde(flatten)]
    _extra: std::collections::BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct RpcLog {
    address: String,
    topics: Vec<String>,
    data: String,
    #[serde(flatten)]
    _extra: std::collections::BTreeMap<String, Value>,
}

fn validate_and_order_receipts(
    receipts: Vec<RpcReceipt>,
    expected_height: u64,
    expected_block_hash: B256,
) -> Result<Vec<RpcReceipt>> {
    anyhow::ensure!(
        !receipts.is_empty() && receipts.len() <= MAX_BLOCK_RECEIPTS,
        "HyperEVM block receipt count is out of bounds"
    );
    let mut indexed = Vec::with_capacity(receipts.len());
    let mut seen_hashes = BTreeSet::new();
    for receipt in receipts {
        let index = usize::try_from(parse_rpc_quantity(
            "receipt.transactionIndex",
            &receipt.transaction_index,
        )?)
        .context("receipt transaction index exceeds usize")?;
        anyhow::ensure!(
            parse_rpc_quantity("receipt.blockNumber", &receipt.block_number)? == expected_height
                && parse_b256("receipt.blockHash", &receipt.block_hash)? == expected_block_hash,
            "HyperEVM RPC returned a receipt from a different block"
        );
        let tx_hash = parse_b256("receipt.transactionHash", &receipt.transaction_hash)?;
        anyhow::ensure!(seen_hashes.insert(tx_hash), "duplicate transaction receipt");
        indexed.push((index, receipt));
    }
    indexed.sort_by_key(|(index, _)| *index);
    for (expected, (actual, _)) in indexed.iter().enumerate() {
        anyhow::ensure!(
            *actual == expected,
            "receipt transaction indexes are duplicated, missing, or non-contiguous"
        );
    }
    Ok(indexed.into_iter().map(|(_, receipt)| receipt).collect())
}

fn select_target_receipt(
    receipts: &[RpcReceipt],
    reader_contract: Address,
    snapshot_tx_hash: B256,
) -> Result<usize> {
    let topic0 = hl_receipt_event_topic0();
    let mut matched = None;
    for (index, receipt) in receipts.iter().enumerate() {
        if parse_b256("receipt.transactionHash", &receipt.transaction_hash)? != snapshot_tx_hash {
            continue;
        }
        anyhow::ensure!(
            matched.is_none(),
            "snapshot transaction receipt is duplicated"
        );
        let status = receipt
            .status
            .as_deref()
            .context("snapshot receipt does not contain a status")?;
        anyhow::ensure!(
            parse_rpc_quantity("snapshot receipt status", status)? == 1,
            "snapshot transaction did not succeed"
        );
        let matching_logs = receipt
            .logs
            .iter()
            .filter(|log| {
                parse_address("receipt log address", &log.address)
                    .is_ok_and(|address| address == reader_contract)
                    && log.topics.first().is_some_and(|topic| {
                        parse_b256("receipt topic0", topic).is_ok_and(|value| value == topic0)
                    })
            })
            .count();
        anyhow::ensure!(
            matching_logs == 1,
            "snapshot receipt must contain exactly one governed reader event"
        );
        matched = Some(index);
    }
    matched.context("exact snapshot transaction was not found in the certified block receipts")
}

fn encode_receipt(receipt: &RpcReceipt) -> Result<Vec<u8>> {
    anyhow::ensure!(
        receipt.logs.len() <= MAX_HYPERLIQUID_RECEIPT_LOGS,
        "receipt log count exceeds verifier bound"
    );
    let status_or_root = match (&receipt.status, &receipt.root) {
        (Some(status), _) => {
            let status_value = parse_rpc_quantity("receipt.status", status)?;
            anyhow::ensure!(status_value <= 1, "receipt status is not zero or one");
            rlp_quantity("receipt.status", status)?
        }
        (None, Some(root)) => rlp_fixed_bytes("receipt.root", root, 32)?,
        (None, None) => bail!("receipt is missing both status and pre-Byzantium root"),
    };
    let cumulative_gas = rlp_quantity("receipt.cumulativeGasUsed", &receipt.cumulative_gas_used)?;
    let logs_bloom = rlp_fixed_bytes("receipt.logsBloom", &receipt.logs_bloom, 256)?;
    let logs = rlp_list(
        receipt
            .logs
            .iter()
            .map(encode_log)
            .collect::<Result<Vec<_>>>()?,
    );
    let body = rlp_list(vec![status_or_root, cumulative_gas, logs_bloom, logs]);
    let receipt_type = receipt
        .tx_type
        .as_deref()
        .map(|value| parse_rpc_quantity("receipt.type", value))
        .transpose()?
        .unwrap_or(0);
    let encoded = if receipt_type == 0 {
        body
    } else {
        anyhow::ensure!(
            receipt_type <= 0x7f,
            "EIP-2718 receipt type exceeds one byte"
        );
        let mut typed = Vec::with_capacity(body.len() + 1);
        typed.push(receipt_type as u8);
        typed.extend(body);
        typed
    };
    anyhow::ensure!(
        encoded.len() <= MAX_HYPERLIQUID_RECEIPT_BYTES,
        "encoded receipt exceeds verifier bound"
    );
    Ok(encoded)
}

fn encode_log(log: &RpcLog) -> Result<Vec<u8>> {
    anyhow::ensure!(
        log.topics.len() <= MAX_HYPERLIQUID_LOG_TOPICS,
        "receipt log topic count exceeds verifier bound"
    );
    let address = rlp_fixed_bytes("receipt log address", &log.address, 20)?;
    let topics = rlp_list(
        log.topics
            .iter()
            .map(|topic| rlp_fixed_bytes("receipt log topic", topic, 32))
            .collect::<Result<Vec<_>>>()?,
    );
    let data = rlp_bytes(&decode_data(
        "receipt log data",
        &log.data,
        MAX_HYPERLIQUID_RECEIPT_BYTES,
    )?);
    Ok(rlp_list(vec![address, topics, data]))
}

fn build_receipts_trie_with_proof(
    encoded_receipts: &[Vec<u8>],
    target_index: usize,
) -> Result<(B256, Vec<alloy_primitives::Bytes>)> {
    anyhow::ensure!(
        !encoded_receipts.is_empty(),
        "cannot prove an empty receipt trie"
    );
    anyhow::ensure!(
        target_index < encoded_receipts.len(),
        "target receipt index is out of bounds"
    );
    let target_key = receipt_trie_key(target_index);
    let retainer = ProofRetainer::new(vec![target_key]);
    let mut builder = HashBuilder::default().with_proof_retainer(retainer);
    let len = encoded_receipts.len();
    for index in 0..len {
        let adjusted = adjust_index_for_rlp(index, len);
        builder.add_leaf(receipt_trie_key(adjusted), &encoded_receipts[adjusted]);
    }
    let root = builder.root();
    let retained = builder.take_proof_nodes();
    let proof = minimal_proof_nodes(
        root,
        target_key,
        &encoded_receipts[target_index],
        retained.matching_nodes_sorted(&target_key),
    )?;
    Ok((root, proof))
}

fn receipt_trie_key(index: usize) -> Nibbles {
    Nibbles::unpack(alloy_rlp::encode_fixed_size(&index))
}

fn minimal_proof_nodes(
    root: B256,
    target_key: Nibbles,
    expected_value: &[u8],
    mut retained_nodes: Vec<(Nibbles, alloy_primitives::Bytes)>,
) -> Result<Vec<alloy_primitives::Bytes>> {
    let mut proof = Vec::new();
    let mut expected_node = RlpNode::word_rlp(&root);
    let mut walked_path = Nibbles::new();
    loop {
        let position = retained_nodes
            .iter()
            .position(|(_, node)| RlpNode::from_rlp(node).as_slice() == expected_node.as_slice())
            .context("retained proof nodes are missing the expected node")?;
        let (_, node) = retained_nodes.remove(position);
        let trie_node = TrieNode::decode(&mut &node[..]).context("decode receipt trie node")?;
        proof.push(node);
        match walk_trie_node(trie_node, &mut walked_path, &target_key)? {
            Some(ProofWalkResult::Node(next)) => expected_node = next,
            Some(ProofWalkResult::Value(value)) => {
                anyhow::ensure!(
                    walked_path == target_key && value == expected_value,
                    "receipt proof did not end at the expected value"
                );
                return Ok(proof);
            }
            None => bail!("receipt proof path terminated before the target"),
        }
    }
}

enum ProofWalkResult {
    Node(RlpNode),
    Value(Vec<u8>),
}

fn walk_trie_node(
    node: TrieNode,
    walked_path: &mut Nibbles,
    key: &Nibbles,
) -> Result<Option<ProofWalkResult>> {
    match node {
        TrieNode::Branch(branch) => walk_branch_node(branch, walked_path, key),
        TrieNode::Extension(extension) => {
            walked_path.extend(&extension.key);
            if extension.child.is_hash() {
                Ok(Some(ProofWalkResult::Node(extension.child)))
            } else {
                walk_trie_node(
                    TrieNode::decode(&mut &extension.child[..])?,
                    walked_path,
                    key,
                )
            }
        }
        TrieNode::Leaf(leaf) => {
            walked_path.extend(&leaf.key);
            Ok(Some(ProofWalkResult::Value(leaf.value)))
        }
        TrieNode::EmptyRoot => bail!("unexpected empty receipt trie root"),
    }
}

fn walk_branch_node(
    mut branch: BranchNode,
    walked_path: &mut Nibbles,
    key: &Nibbles,
) -> Result<Option<ProofWalkResult>> {
    let Some(next) = key.get(walked_path.len()) else {
        return Ok(None);
    };
    let mut stack_ptr = branch.as_ref().first_child_index();
    for index in CHILD_INDEX_RANGE {
        if branch.state_mask.is_bit_set(index) {
            if index == next {
                walked_path.push(next);
                let child = branch.stack.remove(stack_ptr);
                if child.is_hash() {
                    return Ok(Some(ProofWalkResult::Node(child)));
                }
                return walk_trie_node(TrieNode::decode(&mut &child[..])?, walked_path, key);
            }
            stack_ptr += 1;
        }
    }
    Ok(None)
}

fn snapshot_calldata(
    policy: &HyperliquidReceiptPolicyV1,
    owner: Address,
    salt: B256,
) -> Result<Vec<u8>> {
    policy
        .validate()
        .map_err(|error| anyhow!("Hyperliquid policy is invalid: {error:?}"))?;
    let selector = &keccak256(SNAPSHOT_FUNCTION_SIGNATURE)[..4];
    let perps_offset = 4usize
        .checked_mul(32)
        .context("snapshot ABI head size overflow")?;
    let perps_tail_bytes = 32usize
        .checked_add(
            policy
                .required_perps
                .len()
                .checked_mul(32)
                .context("snapshot perp ABI size overflow")?,
        )
        .context("snapshot perp ABI size overflow")?;
    let spots_offset = perps_offset
        .checked_add(perps_tail_bytes)
        .context("snapshot spot ABI offset overflow")?;
    let mut out = Vec::new();
    out.extend_from_slice(selector);
    out.extend_from_slice(&word_address(owner));
    out.extend_from_slice(&word_usize(perps_offset)?);
    out.extend_from_slice(&word_usize(spots_offset)?);
    out.extend_from_slice(salt.as_slice());
    out.extend_from_slice(&word_usize(policy.required_perps.len())?);
    for perp in &policy.required_perps {
        out.extend_from_slice(&word_u64(u64::from(*perp)));
    }
    out.extend_from_slice(&word_usize(policy.allowed_spot_tokens.len())?);
    for spot in &policy.allowed_spot_tokens {
        let (price_asset, price_asset_sz_decimals) = spot_price_request(spot)?;
        out.extend_from_slice(&word_u64(spot.token));
        out.extend_from_slice(&word_u64(u64::from(spot.wei_decimals)));
        out.extend_from_slice(&word_u64(u64::from(price_asset)));
        out.extend_from_slice(&word_u64(u64::from(price_asset_sz_decimals)));
    }
    Ok(out)
}

fn spot_price_request(spot: &HyperliquidSpotTokenPolicyV1) -> Result<(u32, u8)> {
    match (spot.token, spot.wei_decimals) {
        (150, 8) => Ok((159, 2)),
        (404, 8) => Ok((224, 3)),
        _ => bail!("spot token is unsupported by the governed public reader"),
    }
}

fn word_address(value: Address) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[12..].copy_from_slice(value.as_slice());
    word
}

fn word_u64(value: u64) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}

fn word_usize(value: usize) -> Result<[u8; 32]> {
    Ok(word_u64(
        u64::try_from(value).context("ABI value exceeds u64")?,
    ))
}

fn rlp_fixed_bytes(label: &str, value: &str, expected_bytes: usize) -> Result<Vec<u8>> {
    let bytes = decode_data(label, value, expected_bytes)?;
    anyhow::ensure!(
        bytes.len() == expected_bytes,
        "{label} must be exactly {expected_bytes} bytes"
    );
    Ok(rlp_bytes(&bytes))
}

fn rlp_quantity(label: &str, value: &str) -> Result<Vec<u8>> {
    let parsed = parse_rpc_quantity(label, value)?;
    if parsed == 0 {
        return Ok(rlp_bytes(&[]));
    }
    let bytes = parsed.to_be_bytes();
    let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    Ok(rlp_bytes(&bytes[first..]))
}

fn rlp_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 9);
    bytes.encode(&mut out);
    out
}

fn rlp_list(items: Vec<Vec<u8>>) -> Vec<u8> {
    let payload_len = items.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(payload_len + 9);
    Header {
        list: true,
        payload_length: payload_len,
    }
    .encode(&mut out);
    for item in items {
        out.extend(item);
    }
    out
}

fn parse_rpc_quantity(label: &str, value: &str) -> Result<u64> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("{label} is not a 0x-prefixed JSON-RPC quantity"))?;
    anyhow::ensure!(!digits.is_empty(), "{label} is empty");
    anyhow::ensure!(
        digits.len() == 1 || !digits.starts_with('0'),
        "{label} contains a non-canonical leading zero"
    );
    anyhow::ensure!(
        digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} is not canonical lowercase hex"
    );
    parse_u64_quantity(label, value)
}

fn decode_data(label: &str, value: &str, maximum_bytes: usize) -> Result<Vec<u8>> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("{label} is not 0x-prefixed hex data"))?;
    anyhow::ensure!(digits.len() % 2 == 0, "{label} has odd-length hex");
    anyhow::ensure!(
        digits.len() / 2 <= maximum_bytes,
        "{label} exceeds {maximum_bytes} bytes"
    );
    hex::decode(digits).with_context(|| format!("{label} contains non-hex data"))
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

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("missing or non-string block field {field}"))
}

fn has_non_null(value: &Value, field: &str) -> bool {
    value.get(field).is_some_and(|field| !field.is_null())
}

pub(crate) fn fuzz_external_input(data: &[u8]) {
    if data.len() > MAX_HYPEREVM_FUZZ_INPUT_BYTES {
        return;
    }
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_rpc_quantity("fuzz HyperEVM quantity", text);
        let _ = decode_data("fuzz HyperEVM data", text, MAX_HYPEREVM_FUZZ_INPUT_BYTES);
        let _ = rlp_quantity("fuzz HyperEVM RLP quantity", text);
    }
    if let Ok(block) = serde_json::from_slice::<Value>(data) {
        let expected_height = block
            .get("number")
            .and_then(Value::as_str)
            .and_then(|value| parse_rpc_quantity("fuzz block number", value).ok())
            .unwrap_or_default();
        let _ = validate_and_encode_block(&block, expected_height);
    }
    if let Ok(receipts) = serde_json::from_slice::<Vec<RpcReceipt>>(data) {
        if receipts.is_empty() || receipts.len() > MAX_BLOCK_RECEIPTS {
            return;
        }
        let encoded = receipts
            .iter()
            .map(encode_receipt)
            .collect::<Result<Vec<_>>>();
        if let Ok(encoded) = encoded {
            let _ = build_receipts_trie_with_proof(&encoded, 0);
        }
        if let Some(first) = receipts.first() {
            if let (Ok(height), Ok(hash)) = (
                parse_rpc_quantity("fuzz receipt block", &first.block_number),
                parse_b256("fuzz receipt hash", &first.block_hash),
            ) {
                let _ = validate_and_order_receipts(receipts, height, hash);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HISTORICAL_BLOCK: &str = r#"{
      "number":"0x27edb1a",
      "hash":"0x53cab91dfb6c4ef2048ea0a0b6de5ef324f170b1639665fbceb7b404a10885a7",
      "parentHash":"0x3527638683e844a93f0bb16c2c702df127b31d0cfa1f28e03c60b39adc654892",
      "sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
      "miner":"0x0000000000000000000000000000000000000000",
      "stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000",
      "transactionsRoot":"0xa68562df9173fe481966e4dfa2f01d18850558fb58187e0b2ce59a579c289f03",
      "receiptsRoot":"0x6cc0ec13263247e930a6d64c6a37cd31c661dad8dca685e9b0ade33cc5d496a0",
      "logsBloom":"0x00000000000000000000002000000800000000000000000000000000000000000000000000200000000004000010000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000001000800000000000000000000000000000080000000000000000000000000000000000000000000000000000000000400000000000000000000020004080000000000000100000008000000000000000000000000000000000000000000002000000001000000000000000000000000200000000000010000200010000000000000000000000000000000000000000000000000000000000000",
      "difficulty":"0x0",
      "gasLimit":"0x2dc6c0",
      "gasUsed":"0x41763",
      "timestamp":"0x6a6b9da9",
      "extraData":"0x",
      "mixHash":"0x0000000000000000000000000000000000000000000000000000000000000000",
      "nonce":"0x0000000000000000",
      "baseFeePerGas":"0x5f5e100",
      "withdrawalsRoot":"0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
      "blobGasUsed":"0x0",
      "excessBlobGas":"0x0",
      "parentBeaconBlockRoot":"0x0000000000000000000000000000000000000000000000000000000000000000",
      "requestsHash":null
    }"#;

    const HISTORICAL_WITNESS: &str = include_str!(
        "../../../../../docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/hl-receipt-witness.json"
    );

    fn policy() -> HyperliquidReceiptPolicyV1 {
        HyperliquidReceiptPolicyV1 {
            source_domain: "eip155:999".to_string(),
            hyperevm_chain_id: 999,
            reader_contract: Address::repeat_byte(0x11),
            reader_code_hash: B256::repeat_byte(0x22),
            required_perps: vec![0, 7],
            allowed_spot_tokens: vec![
                HyperliquidSpotTokenPolicyV1 {
                    token: 150,
                    wei_decimals: 8,
                },
                HyperliquidSpotTokenPolicyV1 {
                    token: 404,
                    wei_decimals: 8,
                },
            ],
        }
    }

    #[test]
    fn snapshot_calldata_has_exact_selector_offsets_and_rows() {
        let owner = Address::repeat_byte(0x33);
        let salt = B256::repeat_byte(0x44);
        let calldata = snapshot_calldata(&policy(), owner, salt).unwrap();
        assert_eq!(&calldata[..4], &keccak256(SNAPSHOT_FUNCTION_SIGNATURE)[..4]);
        assert_eq!(&calldata[4 + 12..4 + 32], owner.as_slice());
        assert_eq!(&calldata[4 + 32..4 + 64], &word_u64(128));
        assert_eq!(&calldata[4 + 64..4 + 96], &word_u64(224));
        assert_eq!(&calldata[4 + 96..4 + 128], salt.as_slice());
        assert_eq!(&calldata[4 + 128..4 + 160], &word_u64(2));
        assert_eq!(&calldata[4 + 160..4 + 192], &word_u64(0));
        assert_eq!(&calldata[4 + 192..4 + 224], &word_u64(7));
        assert_eq!(&calldata[4 + 224..4 + 256], &word_u64(2));
        assert_eq!(&calldata[4 + 256..4 + 288], &word_u64(150));
        assert_eq!(&calldata[4 + 320..4 + 352], &word_u64(159));
        assert_eq!(&calldata[4 + 352..4 + 384], &word_u64(2));
        assert_eq!(&calldata[4 + 384..4 + 416], &word_u64(404));
        assert_eq!(&calldata[4 + 448..4 + 480], &word_u64(224));
        assert_eq!(&calldata[4 + 480..4 + 512], &word_u64(3));
        assert_eq!(calldata.len(), 4 + 512);
    }

    #[test]
    fn historical_header_encoding_matches_block_hash_and_existing_witness() {
        let block: Value = serde_json::from_str(HISTORICAL_BLOCK).unwrap();
        let encoded = validate_and_encode_block(&block, 41_868_058).unwrap();
        assert_eq!(
            encoded.block_hash,
            B256::from_slice(
                &hex::decode("53cab91dfb6c4ef2048ea0a0b6de5ef324f170b1639665fbceb7b404a10885a7")
                    .unwrap()
            )
        );
        let witness: Value = serde_json::from_str(HISTORICAL_WITNESS).unwrap();
        let historical_header: Vec<u8> =
            serde_json::from_value(witness["block_header_rlp"].clone()).unwrap();
        assert_eq!(encoded.header_rlp, historical_header);
    }

    #[test]
    fn rpc_quantity_parser_rejects_ambiguous_encodings() {
        assert_eq!(parse_rpc_quantity("height", "0x0").unwrap(), 0);
        assert_eq!(parse_rpc_quantity("height", "0x2a").unwrap(), 42);
        assert!(parse_rpc_quantity("height", "2a").is_err());
        assert!(parse_rpc_quantity("height", "0x02").is_err());
        assert!(parse_rpc_quantity("height", "0x2A").is_err());
    }

    #[test]
    fn receipt_ordering_rejects_gap_duplicate_and_cross_block_data() {
        let block_hash = B256::repeat_byte(0x55);
        let receipt = |index: &str, tx_byte: u8| RpcReceipt {
            tx_type: Some("0x2".to_string()),
            status: Some("0x1".to_string()),
            root: None,
            cumulative_gas_used: "0x5208".to_string(),
            logs_bloom: format!("0x{}", "00".repeat(256)),
            logs: Vec::new(),
            transaction_hash: format!("0x{}", hex::encode([tx_byte; 32])),
            transaction_index: index.to_string(),
            block_hash: format!("{block_hash:#x}"),
            block_number: "0x64".to_string(),
            _extra: Default::default(),
        };
        assert!(validate_and_order_receipts(
            vec![receipt("0x1", 2), receipt("0x0", 1)],
            100,
            block_hash
        )
        .is_ok());
        assert!(validate_and_order_receipts(
            vec![receipt("0x0", 1), receipt("0x2", 2)],
            100,
            block_hash
        )
        .is_err());
        assert!(validate_and_order_receipts(
            vec![receipt("0x0", 1), receipt("0x0", 2)],
            100,
            block_hash
        )
        .is_err());
    }

    #[test]
    fn receipt_trie_proof_reconstructs_each_receipt() {
        let receipts = vec![vec![0xc1, 0x01], vec![0xc1, 0x02], vec![0xc1, 0x03]];
        for index in 0..receipts.len() {
            let (root, nodes) = build_receipts_trie_with_proof(&receipts, index).unwrap();
            alloy_trie::proof::verify_proof(
                root,
                receipt_trie_key(index),
                Some(receipts[index].clone()),
                nodes.iter(),
            )
            .unwrap();
        }
    }

    #[test]
    #[ignore = "requires the public HyperEVM historical RPC"]
    fn public_historical_rpc_reconstructs_certified_receipt_root() {
        let client = rpc_client().unwrap();
        let rpc_url = validate_rpc_url("https://rpc.hyperliquid.xyz/evm").unwrap();
        let block: Value = rpc_call(
            &client,
            &rpc_url,
            "eth_getBlockByNumber",
            serde_json::json!(["0x27edb1a", false]),
        )
        .unwrap();
        let encoded_block = validate_and_encode_block(&block, 41_868_058).unwrap();
        let receipts: Vec<RpcReceipt> = rpc_call(
            &client,
            &rpc_url,
            "eth_getBlockReceipts",
            serde_json::json!(["0x27edb1a"]),
        )
        .unwrap();
        let receipts =
            validate_and_order_receipts(receipts, 41_868_058, encoded_block.block_hash).unwrap();
        let reader = parse_address("reader", "0xd5c4200b74929952dca4db70fdc65317c2705207").unwrap();
        let tx = parse_b256(
            "tx",
            "0x4ee9aa62e49977988de57a8a0a2523b606a3bf9e068a0a5288c73790d0dec719",
        )
        .unwrap();
        let target = select_target_receipt(&receipts, reader, tx).unwrap();
        let encoded_receipts = receipts
            .iter()
            .map(encode_receipt)
            .collect::<Result<Vec<_>>>()
            .unwrap();
        let (root, proof) = build_receipts_trie_with_proof(&encoded_receipts, target).unwrap();
        assert_eq!(root, encoded_block.receipts_root);
        alloy_trie::proof::verify_proof(
            root,
            receipt_trie_key(target),
            Some(encoded_receipts[target].clone()),
            proof.iter(),
        )
        .unwrap();
    }
}
