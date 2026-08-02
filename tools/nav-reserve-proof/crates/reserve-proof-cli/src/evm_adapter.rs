use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    net::IpAddr,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use alloy_primitives::{Address, B256, U256};
use anyhow::{bail, Context, Result};
use clap::{Subcommand, ValueEnum};
use reqwest::{blocking::Client, redirect::Policy, Url};
use reserve_proof_types::{
    aave_v3::{
        aave_reserve_storage_slot, aave_v3_owner_authorization_statement_for_policy_v1,
        aave_v3_owner_commitment, chainlink_latest_round, chainlink_transmission_slot,
        current_phase_aggregator, fixed_storage_slot, mapping_slot_address,
        verify_aave_v3_proof_v1, AaveOraclePriceProofV1, AaveOracleSourcePolicyV1,
        AaveOracleSourceProofV1, AaveV3PolicyV1, AaveV3PositionPolicyV1, AaveV3PositionProofV1,
        AaveV3ProofV1, AaveV3ReserveProofV1, AaveV3VerifyContextV1, ChainlinkFeedProofV1,
        AAVE_EVM_CHECKPOINT_KIND_V1, AAVE_V3_ADAPTER_KIND_V1,
    },
    bft_checkpoint::{
        BftCheckpointCommitteeV1, BftSourceCheckpointCertificateV1, BftSourceCheckpointV1,
    },
    ed25519_evidence_signing_statement, ed25519_verifier_commitment,
    evm_checkpoint::{
        erc20_balance_slot, evm_owner_authorization_statement, evm_owner_commitment,
        EvmAccountProofV1, EvmErc20BalanceProofV1, EvmStateCheckpointCertificateV1,
        EvmStateCheckpointV1, EvmStorageProofV1, EVM_ERC20_ADAPTER_KIND_V1,
        MAX_EVM_PROOF_TOTAL_BYTES,
    },
    evm_spot::{
        erc20_balance_slot as spot_balance_slot, evm_spot_owner_authorization_statement_v1,
        evm_spot_owner_commitment, verify_evm_spot_quantity_proof_v1, EvmSpotChainProofV1,
        EvmSpotPolicyV1, EvmSpotQuantityProofV1, EvmSpotTokenProofV1, EvmSpotVerifyContextV1,
        EVM_SPOT_ADAPTER_KIND_V1, EVM_SPOT_CHECKPOINT_KIND_V1,
    },
    verify_observation_evidence, EvidenceDimensionV1, ReserveProofContextV1, SourceEvidenceV1,
    SourceManifestEntryV1, SourceManifestV1, SourceObservationV1, TrustClassV1, MAX_WITNESS_BYTES,
};
use serde::Deserialize;

use crate::hyperliquid_adapter::{self, HyperliquidCommand};
use crate::monero_adapter::{self, MoneroCommand};
use crate::near_adapter::{self, NearCommand};

const MAX_RPC_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Subcommand)]
pub enum AdapterCommand {
    /// Collect Aave V3 collateral, debt, reserve-index, and oracle proofs
    /// beneath an independently certified EVM state root.
    AaveV3 {
        #[command(subcommand)]
        command: AaveV3Command,
    },
    /// Provider-neutral ERC-20 state-proof collection under a governed BFT
    /// checkpoint.
    EvmErc20 {
        #[command(subcommand)]
        command: EvmErc20Command,
    },
    /// Collect the exact governed multichain native/ERC-20 spot set beneath
    /// independently certified EVM state roots.
    EvmSpot {
        #[command(subcommand)]
        command: EvmSpotCommand,
    },
    /// Build and verify public HyperCore reader receipt proofs under a
    /// governed HyperEVM header checkpoint.
    Hyperliquid {
        #[command(subcommand)]
        command: HyperliquidCommand,
    },
    /// Build and verify public staked-NEAR callback receipt proofs under a
    /// governed finalized-head checkpoint.
    Near {
        #[command(subcommand)]
        command: NearCommand,
    },
    /// Collect and verify a public Monero reserve proof and certified
    /// key-image status checkpoint.
    Monero {
        #[command(subcommand)]
        command: MoneroCommand,
    },
    /// Emit the canonical statement for an Ed25519 attestation or protocol
    /// receipt already represented in a source observation.
    Ed25519EvidenceStatement {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        observation: PathBuf,
        #[arg(long)]
        dimension: EvidenceDimensionArg,
        #[arg(long)]
        output: PathBuf,
    },
    /// Attach and verify an Ed25519 evidence signature before writing a new
    /// observation artifact.
    Ed25519EvidenceAttach {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        observation: PathBuf,
        #[arg(long)]
        dimension: EvidenceDimensionArg,
        #[arg(long)]
        signature: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Derive the exact 48-byte manifest commitment for an Ed25519 verifier
    /// public key.
    Ed25519VerifierCommitment {
        #[arg(long)]
        public_key: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AaveV3Command {
    /// Query the governed EVM RPC and emit the deterministic checkpoint a
    /// validator independently reproduces before signing.
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
    /// Emit the EIP-191 statement authorizing the governed Aave policy and
    /// exact certified checkpoint.
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
    /// Query every policy-pinned Aave, token, reserve, and oracle storage
    /// location at the certified block and emit a fully verified observation.
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
        gross_assets: u64,
        #[arg(long)]
        total_liabilities: u64,
        #[arg(long)]
        disclosure_commitment: String,
        #[arg(long)]
        rpc_url: String,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum EvidenceDimensionArg {
    Quantity,
    Valuation,
}

impl From<EvidenceDimensionArg> for EvidenceDimensionV1 {
    fn from(value: EvidenceDimensionArg) -> Self {
        match value {
            EvidenceDimensionArg::Quantity => Self::Quantity,
            EvidenceDimensionArg::Valuation => Self::Valuation,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum EvmErc20Command {
    /// Emit the canonical ML-DSA checkpoint statement for one validator.
    CheckpointVoteStatement {
        #[arg(long)]
        checkpoint: PathBuf,
        #[arg(long)]
        validator_id: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Emit the exact EIP-191 message the reserve owner must sign.
    OwnerAuthorization {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        committee_root: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Query eth_getBlockByNumber and eth_getProof, then emit a verified
    /// source observation. HTTPS is required except for loopback development.
    Collect {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        checkpoint_certificate: PathBuf,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        ownership_signature: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        balance_slot_index: String,
        /// A complete, separately classified valuation evidence object.
        #[arg(long)]
        valuation_evidence: PathBuf,
        #[arg(long)]
        gross_assets: u64,
        #[arg(long, default_value_t = 0)]
        total_liabilities: u64,
        #[arg(long)]
        disclosure_commitment: String,
        #[arg(long)]
        ethereum_rpc_url: String,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum EvmSpotCommand {
    /// Query one governed EVM RPC and emit the deterministic checkpoint a
    /// validator may independently reproduce before signing.
    CheckpointCandidate {
        #[arg(long)]
        pftl_genesis_hash: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        source_domain: String,
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
    /// Emit the EIP-191 statement authorizing the exact policy and certified
    /// source checkpoints for one observation.
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
        owner: String,
        #[arg(long, required = true, num_args = 1..)]
        checkpoint_certificate: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Query each governed EVM source at its certified block and emit a
    /// complete quantity observation. RPC URLs come from a reviewed map file.
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
        owner: String,
        #[arg(long)]
        ownership_signature: String,
        #[arg(long, required = true, num_args = 1..)]
        checkpoint_certificate: Vec<PathBuf>,
        #[arg(long)]
        rpc_map: PathBuf,
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

pub fn run(command: AdapterCommand) -> Result<()> {
    match command {
        AdapterCommand::AaveV3 { command } => match command {
            AaveV3Command::CheckpointCandidate {
                pftl_genesis_hash,
                policy,
                source_height,
                minimum_depth,
                pftl_observation_height,
                committee,
                rpc_url,
                output,
            } => aave_v3_checkpoint_candidate(AaveV3CheckpointCandidateArgs {
                pftl_genesis_hash,
                policy,
                source_height,
                minimum_depth,
                pftl_observation_height,
                committee,
                rpc_url,
                output,
            }),
            AaveV3Command::OwnerAuthorization {
                manifest,
                context,
                source_id,
                policy,
                checkpoint_certificate,
                owner,
                output,
            } => aave_v3_owner_authorization(AaveV3OwnerAuthorizationArgs {
                manifest,
                context,
                source_id,
                policy,
                checkpoint_certificate,
                owner,
                output,
            }),
            AaveV3Command::Collect {
                manifest,
                context,
                source_id,
                policy,
                checkpoint_certificate,
                owner,
                ownership_signature,
                gross_assets,
                total_liabilities,
                disclosure_commitment,
                rpc_url,
                output,
            } => aave_v3_collect(AaveV3CollectArgs {
                manifest,
                context,
                source_id,
                policy,
                checkpoint_certificate,
                owner,
                ownership_signature,
                gross_assets,
                total_liabilities,
                disclosure_commitment,
                rpc_url,
                output,
            }),
        },
        AdapterCommand::EvmErc20 { command } => match command {
            EvmErc20Command::CheckpointVoteStatement {
                checkpoint,
                validator_id,
                output,
            } => checkpoint_vote_statement(checkpoint, validator_id, output),
            EvmErc20Command::OwnerAuthorization {
                manifest,
                context,
                source_id,
                owner,
                token,
                committee_root,
                output,
            } => owner_authorization(
                manifest,
                context,
                source_id,
                owner,
                token,
                committee_root,
                output,
            ),
            EvmErc20Command::Collect {
                manifest,
                context,
                source_id,
                checkpoint_certificate,
                owner,
                ownership_signature,
                token,
                balance_slot_index,
                valuation_evidence,
                gross_assets,
                total_liabilities,
                disclosure_commitment,
                ethereum_rpc_url,
                output,
            } => collect(CollectArgs {
                manifest,
                context,
                source_id,
                checkpoint_certificate,
                owner,
                ownership_signature,
                token,
                balance_slot_index,
                valuation_evidence,
                gross_assets,
                total_liabilities,
                disclosure_commitment,
                ethereum_rpc_url,
                output,
            }),
        },
        AdapterCommand::EvmSpot { command } => match command {
            EvmSpotCommand::CheckpointCandidate {
                pftl_genesis_hash,
                policy,
                source_domain,
                source_height,
                minimum_depth,
                pftl_observation_height,
                committee,
                rpc_url,
                output,
            } => evm_spot_checkpoint_candidate(EvmSpotCheckpointCandidateArgs {
                pftl_genesis_hash,
                policy,
                source_domain,
                source_height,
                minimum_depth,
                pftl_observation_height,
                committee,
                rpc_url,
                output,
            }),
            EvmSpotCommand::OwnerAuthorization {
                manifest,
                context,
                source_id,
                policy,
                owner,
                checkpoint_certificate,
                output,
            } => evm_spot_owner_authorization(EvmSpotOwnerAuthorizationArgs {
                manifest,
                context,
                source_id,
                policy,
                owner,
                checkpoint_certificates: checkpoint_certificate,
                output,
            }),
            EvmSpotCommand::Collect {
                manifest,
                context,
                source_id,
                policy,
                owner,
                ownership_signature,
                checkpoint_certificate,
                rpc_map,
                valuation_evidence,
                gross_assets,
                total_liabilities,
                disclosure_commitment,
                output,
            } => evm_spot_collect(EvmSpotCollectArgs {
                manifest,
                context,
                source_id,
                policy,
                owner,
                ownership_signature,
                checkpoint_certificates: checkpoint_certificate,
                rpc_map,
                valuation_evidence,
                gross_assets,
                total_liabilities,
                disclosure_commitment,
                output,
            }),
        },
        AdapterCommand::Hyperliquid { command } => hyperliquid_adapter::run(command),
        AdapterCommand::Near { command } => near_adapter::run(command),
        AdapterCommand::Monero { command } => monero_adapter::run(command),
        AdapterCommand::Ed25519EvidenceStatement {
            manifest,
            context,
            source_id,
            observation,
            dimension,
            output,
        } => ed25519_statement(
            manifest,
            context,
            source_id,
            observation,
            dimension.into(),
            output,
        ),
        AdapterCommand::Ed25519EvidenceAttach {
            manifest,
            context,
            source_id,
            observation,
            dimension,
            signature,
            output,
        } => ed25519_attach(
            manifest,
            context,
            source_id,
            observation,
            dimension.into(),
            signature,
            output,
        ),
        AdapterCommand::Ed25519VerifierCommitment { public_key, output } => {
            let commitment =
                ed25519_verifier_commitment(&public_key).map_err(anyhow::Error::msg)?;
            if let Some(output) = output {
                write_new(&output, commitment.as_bytes())?;
            }
            println!("{commitment}");
            Ok(())
        }
    }
}

fn ed25519_statement(
    manifest_path: PathBuf,
    context_path: PathBuf,
    source_id: String,
    observation_path: PathBuf,
    dimension: EvidenceDimensionV1,
    output: PathBuf,
) -> Result<()> {
    let (_, context, entry) = load_source(&manifest_path, &context_path, &source_id)?;
    let observation: SourceObservationV1 = read_json(&observation_path)?;
    let statement = ed25519_evidence_signing_statement(&context, &entry, &observation, dimension)
        .map_err(anyhow::Error::msg)?;
    write_new(&output, &statement)?;
    print_report(
        "postfiat.reserve_ed25519_evidence_statement.v1",
        &output,
        &statement,
    )
}

fn ed25519_attach(
    manifest_path: PathBuf,
    context_path: PathBuf,
    source_id: String,
    observation_path: PathBuf,
    dimension: EvidenceDimensionV1,
    signature: String,
    output: PathBuf,
) -> Result<()> {
    validate_hex("signature", &signature, 64)?;
    let (_, context, entry) = load_source(&manifest_path, &context_path, &source_id)?;
    let mut observation: SourceObservationV1 = read_json(&observation_path)?;
    let evidence = match dimension {
        EvidenceDimensionV1::Quantity => &mut observation.quantity_evidence,
        EvidenceDimensionV1::Valuation => &mut observation.valuation_evidence,
    };
    match evidence {
        SourceEvidenceV1::AttestedEd25519 {
            signature: current, ..
        }
        | SourceEvidenceV1::ProtocolReceiptEd25519 {
            signature: current, ..
        } => *current = signature,
        _ => bail!("selected evidence dimension is not Ed25519 signed evidence"),
    }
    verify_observation_evidence(&context, &entry, &observation, dimension)
        .map_err(anyhow::Error::msg)?;
    write_new(&output, &serde_json::to_vec_pretty(&observation)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_ed25519_evidence_attachment.v1",
            "output": output,
            "source_id": source_id,
            "dimension": dimension.as_str(),
            "signature_valid": true,
        }))?
    );
    Ok(())
}

fn checkpoint_vote_statement(
    checkpoint_path: PathBuf,
    validator_id: String,
    output: PathBuf,
) -> Result<()> {
    let checkpoint: EvmStateCheckpointV1 = read_json(&checkpoint_path)?;
    let statement = checkpoint
        .vote_signing_statement(&validator_id)
        .map_err(anyhow::Error::msg)?;
    write_new(&output, &statement)?;
    print_report(
        "postfiat.reserve_evm_checkpoint_vote_statement.v1",
        &output,
        &statement,
    )
}

#[allow(clippy::too_many_arguments)]
fn owner_authorization(
    manifest_path: PathBuf,
    context_path: PathBuf,
    source_id: String,
    owner: String,
    token: String,
    committee_root: String,
    output: PathBuf,
) -> Result<()> {
    let (manifest, context, entry) = load_source(&manifest_path, &context_path, &source_id)?;
    let owner = parse_address("owner", &owner)?;
    let token = parse_address("token", &token)?;
    validate_evm_manifest_entry(&entry, owner, token, &committee_root)?;
    let statement = evm_owner_authorization_statement(
        &context.pftl_genesis_hash,
        &context.nav_asset_id,
        &context.proof_profile_id,
        &context.valuation_policy_hash,
        &context.source_manifest_hash,
        &entry.source_id,
        owner,
        token,
        &committee_root,
    )
    .map_err(anyhow::Error::msg)?;
    // Keep the loaded manifest alive through all consistency checks and make
    // the binding explicit to reviewers.
    anyhow::ensure!(
        manifest.hash().map_err(anyhow::Error::msg)? == context.source_manifest_hash,
        "manifest/context hash changed during authorization"
    );
    write_new(&output, &statement)?;
    print_report(
        "postfiat.reserve_evm_owner_authorization.v1",
        &output,
        &statement,
    )
}

struct CollectArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    checkpoint_certificate: PathBuf,
    owner: String,
    ownership_signature: String,
    token: String,
    balance_slot_index: String,
    valuation_evidence: PathBuf,
    gross_assets: u64,
    total_liabilities: u64,
    disclosure_commitment: String,
    ethereum_rpc_url: String,
    output: PathBuf,
}

struct AaveV3CheckpointCandidateArgs {
    pftl_genesis_hash: String,
    policy: PathBuf,
    source_height: u64,
    minimum_depth: u32,
    pftl_observation_height: u64,
    committee: PathBuf,
    rpc_url: String,
    output: PathBuf,
}

struct AaveV3OwnerAuthorizationArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    checkpoint_certificate: PathBuf,
    owner: String,
    output: PathBuf,
}

struct AaveV3CollectArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    checkpoint_certificate: PathBuf,
    owner: String,
    ownership_signature: String,
    gross_assets: u64,
    total_liabilities: u64,
    disclosure_commitment: String,
    rpc_url: String,
    output: PathBuf,
}

fn aave_v3_checkpoint_candidate(args: AaveV3CheckpointCandidateArgs) -> Result<()> {
    validate_hex("pftl_genesis_hash", &args.pftl_genesis_hash, 48)?;
    anyhow::ensure!(
        args.source_height > 0 && args.minimum_depth > 0 && args.pftl_observation_height > 0,
        "checkpoint heights and minimum depth must be nonzero"
    );
    let policy: AaveV3PolicyV1 = read_json(&args.policy)?;
    policy
        .validate()
        .map_err(|error| anyhow::anyhow!("Aave V3 policy is invalid: {error:?}"))?;
    let committee: BftCheckpointCommitteeV1 = read_json(&args.committee)?;
    let committee_root = committee.root().map_err(anyhow::Error::msg)?;
    policy
        .commitment(&committee_root)
        .map_err(|error| anyhow::anyhow!("Aave V3 policy commitment failed: {error:?}"))?;
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    let client = Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()?;
    let chain_id: String = rpc_call(&client, &rpc_url, "eth_chainId", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_u64_quantity("eth_chainId", &chain_id)? == policy.ethereum_chain_id,
        "Aave RPC chain ID does not match policy"
    );
    let observed_head_raw: String =
        rpc_call(&client, &rpc_url, "eth_blockNumber", serde_json::json!([]))?;
    let observed_source_head = parse_u64_quantity("latest block", &observed_head_raw)?;
    let required_head = args
        .source_height
        .checked_add(u64::from(args.minimum_depth))
        .context("Aave checkpoint confirmation depth overflows")?;
    anyhow::ensure!(
        observed_source_head >= required_head,
        "Aave source block has not reached the required confirmation depth"
    );
    let block_tag = format!("0x{:x}", args.source_height);
    let block: RpcBlock = rpc_call(
        &client,
        &rpc_url,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    anyhow::ensure!(
        parse_u64_quantity("block.number", &block.number)? == args.source_height,
        "Aave RPC substituted a different source block"
    );
    let source_timestamp_ms = parse_u64_quantity("block.timestamp", &block.timestamp)?
        .checked_mul(1_000)
        .context("Aave block timestamp milliseconds overflow")?;
    let checkpoint = BftSourceCheckpointV1 {
        pftl_genesis_hash: args.pftl_genesis_hash,
        checkpoint_kind: AAVE_EVM_CHECKPOINT_KIND_V1.to_string(),
        source_domain: policy.source_domain,
        source_height: args.source_height,
        source_timestamp_ms,
        source_block_hash: parse_b256("block.hash", &block.hash)?,
        source_state_commitment: parse_b256("block.stateRoot", &block.state_root)?,
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
            "schema": "postfiat.reserve_aave_v3_checkpoint_candidate.v1",
            "output": args.output,
            "source_domain": checkpoint.source_domain,
            "source_height": checkpoint.source_height,
            "source_timestamp_ms": checkpoint.source_timestamp_ms,
            "source_block_hash": checkpoint.source_block_hash,
            "source_state_commitment": checkpoint.source_state_commitment,
            "observed_source_head": checkpoint.observed_source_head,
            "minimum_depth": checkpoint.minimum_depth,
            "committee_epoch": checkpoint.committee_epoch,
            "committee_root": checkpoint.committee_root,
            "next_required_check": "each validator independently reproduces this candidate before signing its vote statement",
        }))?
    );
    Ok(())
}

fn aave_v3_owner_authorization(args: AaveV3OwnerAuthorizationArgs) -> Result<()> {
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: AaveV3PolicyV1 = read_json(&args.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    let owner = parse_address("owner", &args.owner)?;
    validate_aave_manifest_entry(
        &entry,
        &policy,
        &certificate,
        owner,
        &context.pftl_genesis_hash,
    )?;
    let checkpoint = &certificate.checkpoint;
    let observed_at_pftl_height = checkpoint.pftl_observation_height;
    let verify_context = AaveV3VerifyContextV1 {
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
        expected_gross_assets: 0,
        expected_total_liabilities: 0,
        expected_evidence_commitment: &"00".repeat(48),
    };
    let statement = aave_v3_owner_authorization_statement_for_policy_v1(
        &policy,
        &certificate,
        owner,
        &verify_context,
    )
    .map_err(|error| anyhow::anyhow!("Aave V3 owner statement failed: {error:?}"))?;
    write_new(&args.output, &statement)?;
    print_report(
        "postfiat.reserve_aave_v3_owner_authorization.v1",
        &args.output,
        &statement,
    )
}

fn aave_v3_collect(args: AaveV3CollectArgs) -> Result<()> {
    anyhow::ensure!(
        args.total_liabilities <= args.gross_assets,
        "total liabilities exceed gross assets"
    );
    validate_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: AaveV3PolicyV1 = read_json(&args.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    let owner = parse_address("owner", &args.owner)?;
    validate_aave_manifest_entry(
        &entry,
        &policy,
        &certificate,
        owner,
        &context.pftl_genesis_hash,
    )?;
    let checkpoint = &certificate.checkpoint;
    let observed_at_pftl_height = checkpoint.pftl_observation_height;
    anyhow::ensure!(
        observed_at_pftl_height >= context.observation_not_before
            && observed_at_pftl_height <= context.observation_not_after,
        "Aave checkpoint PFTL height is outside the observation interval"
    );
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    let client = Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()?;
    validate_certified_aave_rpc(&client, &rpc_url, &policy, checkpoint)?;
    let block_tag = format!("0x{:x}", checkpoint.source_height);
    let mut positions = Vec::with_capacity(policy.positions.len());
    for position in &policy.positions {
        positions.push(collect_aave_position(
            &client, &rpc_url, &block_tag, &policy, position, owner,
        )?);
    }
    let proof = AaveV3ProofV1 {
        policy,
        checkpoint_certificate: certificate,
        owner,
        ownership_signature: decode_hex("ownership_signature", &args.ownership_signature, 65)?,
        positions,
    };
    let evidence_commitment = proof
        .commitment()
        .map_err(|error| anyhow::anyhow!("Aave V3 evidence commitment failed: {error:?}"))?;
    let verify_context = AaveV3VerifyContextV1 {
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
        expected_gross_assets: args.gross_assets,
        expected_total_liabilities: args.total_liabilities,
        expected_evidence_commitment: &evidence_commitment,
    };
    let verified = verify_aave_v3_proof_v1(&proof, &verify_context)
        .map_err(|error| anyhow::anyhow!("Aave V3 proof failed: {error:?}"))?;
    let evidence = SourceEvidenceV1::AaveV3 {
        evidence_commitment: evidence_commitment.clone(),
        proof: Box::new(proof),
    };
    let observation = SourceObservationV1 {
        source_id: entry.source_id.clone(),
        observed_at_block: observed_at_pftl_height,
        gross_assets: verified.collateral_usd_e8,
        total_liabilities: verified.liability_usd_e8,
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
            "schema": "postfiat.reserve_aave_v3_collection.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "ethereum_block_number": verified.block_number,
            "pftl_observation_height": observation.observed_at_block,
            "collateral_usd_e8": verified.collateral_usd_e8,
            "liability_usd_e8": verified.liability_usd_e8,
            "position_count": proof_position_count(&observation.quantity_evidence),
            "evidence_commitment": evidence_commitment,
            "quantity_trust": "cryptographic_bft_checkpoint_mpt",
            "valuation_trust": "cryptographic_bft_checkpoint_mpt_chainlink",
            "next_required_check": "postfiat-reserve-proof observe validates this source again in the complete manifest",
        }))?
    );
    Ok(())
}

fn proof_position_count(evidence: &SourceEvidenceV1) -> usize {
    match evidence {
        SourceEvidenceV1::AaveV3 { proof, .. } => proof.positions.len(),
        _ => 0,
    }
}

fn validate_aave_manifest_entry(
    entry: &SourceManifestEntryV1,
    policy: &AaveV3PolicyV1,
    certificate: &BftSourceCheckpointCertificateV1,
    owner: Address,
    pftl_genesis_hash: &str,
) -> Result<()> {
    policy
        .validate()
        .map_err(|error| anyhow::anyhow!("Aave V3 policy is invalid: {error:?}"))?;
    certificate.verify().map_err(anyhow::Error::msg)?;
    let checkpoint = &certificate.checkpoint;
    anyhow::ensure!(
        checkpoint.pftl_genesis_hash == pftl_genesis_hash
            && checkpoint.checkpoint_kind == AAVE_EVM_CHECKPOINT_KIND_V1
            && checkpoint.source_domain == policy.source_domain,
        "Aave checkpoint does not match the chain, kind, or policy"
    );
    let committee_root = certificate.committee.root().map_err(anyhow::Error::msg)?;
    let policy_commitment = policy
        .commitment(&committee_root)
        .map_err(|error| anyhow::anyhow!("Aave V3 policy commitment failed: {error:?}"))?;
    anyhow::ensure!(
        entry.adapter_kind == AAVE_V3_ADAPTER_KIND_V1 && entry.adapter_schema_version == 1,
        "source does not use {AAVE_V3_ADAPTER_KIND_V1} schema 1"
    );
    anyhow::ensure!(
        entry.quantity_evidence_class == TrustClassV1::Cryptographic
            && entry.valuation_evidence_class == TrustClassV1::Cryptographic,
        "Aave quantity and valuation must both be classified cryptographic"
    );
    anyhow::ensure!(
        entry.source_domain == policy.source_domain
            && entry.asset_or_position_id == format!("aave-v3:account:{owner:#x}")
            && entry.reserve_owner_commitment == aave_v3_owner_commitment(owner)
            && entry.quantity_verifier_commitment == policy_commitment
            && entry.valuation_verifier_commitment == policy_commitment,
        "Aave manifest identity, owner, or policy commitment mismatch"
    );
    Ok(())
}

fn validate_certified_aave_rpc(
    client: &Client,
    rpc_url: &Url,
    policy: &AaveV3PolicyV1,
    checkpoint: &BftSourceCheckpointV1,
) -> Result<()> {
    let chain_id: String = rpc_call(client, rpc_url, "eth_chainId", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_u64_quantity("eth_chainId", &chain_id)? == policy.ethereum_chain_id,
        "Aave RPC chain ID does not match policy"
    );
    let block_tag = format!("0x{:x}", checkpoint.source_height);
    let block: RpcBlock = rpc_call(
        client,
        rpc_url,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    let timestamp_ms = parse_u64_quantity("block.timestamp", &block.timestamp)?
        .checked_mul(1_000)
        .context("Aave block timestamp milliseconds overflow")?;
    anyhow::ensure!(
        parse_u64_quantity("block.number", &block.number)? == checkpoint.source_height
            && parse_b256("block.hash", &block.hash)? == checkpoint.source_block_hash
            && parse_b256("block.stateRoot", &block.state_root)?
                == checkpoint.source_state_commitment
            && timestamp_ms == checkpoint.source_timestamp_ms,
        "Aave RPC block does not match the certified checkpoint"
    );
    let latest: String = rpc_call(client, rpc_url, "eth_blockNumber", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_u64_quantity("latest block", &latest)? >= checkpoint.observed_source_head,
        "Aave RPC head is behind the certified observation head"
    );
    Ok(())
}

fn collect_aave_position(
    client: &Client,
    rpc_url: &Url,
    block_tag: &str,
    policy: &AaveV3PolicyV1,
    position: &AaveV3PositionPolicyV1,
    owner: Address,
) -> Result<AaveV3PositionProofV1> {
    let user_key = mapping_slot_address(owner, position.user_state_slot_index);
    let (token_account, mut token_storage) = rpc_account_with_storage(
        client,
        rpc_url,
        position.token_address,
        &[user_key],
        block_tag,
    )?;
    let user_state = take_storage(&mut token_storage, user_key, "Aave user state")?;

    let reserve_base = U256::from(policy.reserve_mapping_slot_index);
    let reserve_keys = [1u64, 2, 3, 4, 6]
        .map(|offset| aave_reserve_storage_slot(position.underlying_asset, reserve_base, offset));
    let (pool_account, mut pool_storage) = rpc_account_with_storage(
        client,
        rpc_url,
        policy.pool_address,
        &reserve_keys,
        block_tag,
    )?;
    let reserve = AaveV3ReserveProofV1 {
        pool_account,
        indexes_and_rates_1: take_storage(
            &mut pool_storage,
            reserve_keys[0],
            "Aave reserve indexes/rates 1",
        )?,
        indexes_and_rates_2: take_storage(
            &mut pool_storage,
            reserve_keys[1],
            "Aave reserve indexes/rates 2",
        )?,
        metadata: take_storage(&mut pool_storage, reserve_keys[2], "Aave reserve metadata")?,
        a_token_address: take_storage(&mut pool_storage, reserve_keys[3], "Aave aToken address")?,
        variable_debt_token_address: take_storage(
            &mut pool_storage,
            reserve_keys[4],
            "Aave variable-debt token address",
        )?,
    };

    let source_key = mapping_slot_address(
        position.underlying_asset,
        U256::from(policy.oracle_sources_slot_index),
    );
    let (oracle_account, mut oracle_storage) = rpc_account_with_storage(
        client,
        rpc_url,
        policy.oracle_address,
        &[source_key],
        block_tag,
    )?;
    let source = take_storage(&mut oracle_storage, source_key, "Aave oracle source")?;
    let source_address = address_from_storage("Aave oracle source", source.value)?;
    let (proxy_address, source_kind) = match &position.oracle_source {
        AaveOracleSourcePolicyV1::DirectChainlink { proxy_address } => {
            anyhow::ensure!(
                source_address == *proxy_address,
                "Aave oracle source does not match direct Chainlink policy"
            );
            (*proxy_address, AaveOracleSourceProofV1::DirectChainlink)
        }
        AaveOracleSourcePolicyV1::CappedStable {
            adapter_address,
            chainlink_proxy_address,
            price_cap_slot_index,
            ..
        } => {
            anyhow::ensure!(
                source_address == *adapter_address,
                "Aave oracle source does not match capped-stable policy"
            );
            let cap_key = fixed_storage_slot(*price_cap_slot_index);
            let (adapter_account, mut adapter_storage) =
                rpc_account_with_storage(client, rpc_url, *adapter_address, &[cap_key], block_tag)?;
            let price_cap = take_storage(
                &mut adapter_storage,
                cap_key,
                "Aave capped-stable price cap",
            )?;
            (
                *chainlink_proxy_address,
                AaveOracleSourceProofV1::CappedStable {
                    adapter_account: Box::new(adapter_account),
                    price_cap: Box::new(price_cap),
                },
            )
        }
    };

    let phase_key = fixed_storage_slot(policy.chainlink_proxy_phase_slot_index);
    let (proxy_account, mut proxy_storage) =
        rpc_account_with_storage(client, rpc_url, proxy_address, &[phase_key], block_tag)?;
    let current_phase = take_storage(&mut proxy_storage, phase_key, "Chainlink current phase")?;
    let aggregator = current_phase_aggregator(current_phase.value)
        .map_err(|error| anyhow::anyhow!("Chainlink phase decoding failed: {error:?}"))?;
    let hot_key = fixed_storage_slot(policy.chainlink_hot_vars_slot_index);
    let (_, mut first_hot_storage) =
        rpc_account_with_storage(client, rpc_url, aggregator, &[hot_key], block_tag)?;
    let first_hot = take_storage(&mut first_hot_storage, hot_key, "Chainlink hot variables")?;
    let latest_round = chainlink_latest_round(first_hot.value)
        .map_err(|error| anyhow::anyhow!("Chainlink latest round decoding failed: {error:?}"))?;
    let transmission_key =
        chainlink_transmission_slot(latest_round, policy.chainlink_transmissions_slot_index);
    let (aggregator_account, mut aggregator_storage) = rpc_account_with_storage(
        client,
        rpc_url,
        aggregator,
        &[hot_key, transmission_key],
        block_tag,
    )?;
    let hot_vars = take_storage(&mut aggregator_storage, hot_key, "Chainlink hot variables")?;
    anyhow::ensure!(
        hot_vars.value == first_hot.value,
        "Chainlink hot variables changed within a pinned-block collection"
    );
    let transmission = take_storage(
        &mut aggregator_storage,
        transmission_key,
        "Chainlink transmission",
    )?;
    Ok(AaveV3PositionProofV1 {
        position_id: position.position_id.clone(),
        token_account,
        user_state_slot_index: position.user_state_slot_index,
        user_state,
        reserve,
        oracle: AaveOraclePriceProofV1 {
            oracle_account,
            source,
            source_kind,
            chainlink: ChainlinkFeedProofV1 {
                proxy_account,
                current_phase,
                aggregator_account,
                hot_vars,
                transmission,
                decimals: 8,
            },
        },
    })
}

fn rpc_account_with_storage(
    client: &Client,
    rpc_url: &Url,
    address: Address,
    keys: &[B256],
    block_tag: &str,
) -> Result<(EvmAccountProofV1, BTreeMap<B256, EvmStorageProofV1>)> {
    let key_params = keys
        .iter()
        .map(|key| format!("{key:#x}"))
        .collect::<Vec<_>>();
    let rpc_proof: RpcAccountProof = rpc_call(
        client,
        rpc_url,
        "eth_getProof",
        serde_json::json!([format!("{address:#x}"), key_params, block_tag]),
    )?;
    anyhow::ensure!(
        parse_address("eth_getProof.address", &rpc_proof.address)? == address
            && rpc_proof.storage_proof.len() == keys.len(),
        "eth_getProof returned a substituted account or storage set"
    );
    let expected = keys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    anyhow::ensure!(
        expected.len() == keys.len(),
        "requested storage proof keys contain duplicates"
    );
    let mut storage = BTreeMap::new();
    for item in &rpc_proof.storage_proof {
        let key = parse_b256("storageProof.key", &item.key)?;
        anyhow::ensure!(
            expected.contains(&key) && !storage.contains_key(&key),
            "eth_getProof returned an unexpected or duplicate storage key"
        );
        storage.insert(
            key,
            EvmStorageProofV1 {
                key,
                value: parse_u256("storageProof.value", &item.value)?,
                proof: decode_proof_nodes("storageProof", &item.proof)?,
            },
        );
    }
    let account = rpc_account_proof("account", rpc_proof)?;
    Ok((account, storage))
}

fn take_storage(
    storage: &mut BTreeMap<B256, EvmStorageProofV1>,
    key: B256,
    label: &str,
) -> Result<EvmStorageProofV1> {
    storage
        .remove(&key)
        .with_context(|| format!("{label} proof is missing"))
}

fn address_from_storage(label: &str, value: U256) -> Result<Address> {
    let bytes = value.to_be_bytes::<32>();
    anyhow::ensure!(
        bytes[..12].iter().all(|byte| *byte == 0),
        "{label} contains non-address high bits"
    );
    Ok(Address::from_slice(&bytes[12..]))
}

struct EvmSpotOwnerAuthorizationArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    owner: String,
    checkpoint_certificates: Vec<PathBuf>,
    output: PathBuf,
}

struct EvmSpotCheckpointCandidateArgs {
    pftl_genesis_hash: String,
    policy: PathBuf,
    source_domain: String,
    source_height: u64,
    minimum_depth: u32,
    pftl_observation_height: u64,
    committee: PathBuf,
    rpc_url: String,
    output: PathBuf,
}

struct EvmSpotCollectArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    owner: String,
    ownership_signature: String,
    checkpoint_certificates: Vec<PathBuf>,
    rpc_map: PathBuf,
    valuation_evidence: PathBuf,
    gross_assets: u64,
    total_liabilities: u64,
    disclosure_commitment: String,
    output: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvmSpotRpcMapV1 {
    schema: String,
    sources: BTreeMap<String, String>,
}

fn evm_spot_checkpoint_candidate(args: EvmSpotCheckpointCandidateArgs) -> Result<()> {
    validate_hex("pftl_genesis_hash", &args.pftl_genesis_hash, 48)?;
    anyhow::ensure!(
        args.source_height > 0 && args.minimum_depth > 0 && args.pftl_observation_height > 0,
        "checkpoint heights and minimum depth must be nonzero"
    );
    let policy: EvmSpotPolicyV1 = read_json(&args.policy)?;
    policy
        .validate()
        .map_err(|error| anyhow::anyhow!("EVM spot policy is invalid: {error:?}"))?;
    let chain_policy = policy
        .chains
        .iter()
        .find(|chain| chain.source_domain == args.source_domain)
        .context("source domain is not governed by the EVM spot policy")?;
    let committee: BftCheckpointCommitteeV1 = read_json(&args.committee)?;
    let committee_root = committee.root().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        committee_root == chain_policy.committee_root,
        "checkpoint committee does not match the governed chain policy"
    );
    let rpc_url = validate_rpc_url(&args.rpc_url)?;
    let client = Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()?;
    let chain_id: String = rpc_call(&client, &rpc_url, "eth_chainId", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_u64_quantity("eth_chainId", &chain_id)? == chain_policy.chain_id,
        "EVM RPC chain ID does not match policy"
    );
    let observed_head_raw: String =
        rpc_call(&client, &rpc_url, "eth_blockNumber", serde_json::json!([]))?;
    let observed_source_head = parse_u64_quantity("latest block", &observed_head_raw)?;
    let required_head = args
        .source_height
        .checked_add(u64::from(args.minimum_depth))
        .context("checkpoint confirmation depth overflows")?;
    anyhow::ensure!(
        observed_source_head >= required_head,
        "EVM source block has not reached the required confirmation depth"
    );
    let block_tag = format!("0x{:x}", args.source_height);
    let block: RpcBlock = rpc_call(
        &client,
        &rpc_url,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    anyhow::ensure!(
        parse_u64_quantity("block.number", &block.number)? == args.source_height,
        "EVM RPC substituted a different source block"
    );
    let source_timestamp_ms = parse_u64_quantity("block.timestamp", &block.timestamp)?
        .checked_mul(1_000)
        .context("EVM block timestamp milliseconds overflow")?;
    let checkpoint = BftSourceCheckpointV1 {
        pftl_genesis_hash: args.pftl_genesis_hash,
        checkpoint_kind: EVM_SPOT_CHECKPOINT_KIND_V1.to_string(),
        source_domain: chain_policy.source_domain.clone(),
        source_height: args.source_height,
        source_timestamp_ms,
        source_block_hash: parse_b256("block.hash", &block.hash)?,
        source_state_commitment: parse_b256("block.stateRoot", &block.state_root)?,
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
            "schema": "postfiat.reserve_evm_spot_checkpoint_candidate.v1",
            "output": args.output,
            "source_domain": checkpoint.source_domain,
            "source_height": checkpoint.source_height,
            "source_timestamp_ms": checkpoint.source_timestamp_ms,
            "source_block_hash": checkpoint.source_block_hash,
            "source_state_commitment": checkpoint.source_state_commitment,
            "observed_source_head": checkpoint.observed_source_head,
            "minimum_depth": checkpoint.minimum_depth,
            "committee_epoch": checkpoint.committee_epoch,
            "committee_root": checkpoint.committee_root,
            "next_required_check": "each validator independently reproduces this candidate before signing its vote statement",
        }))?
    );
    Ok(())
}

fn evm_spot_owner_authorization(args: EvmSpotOwnerAuthorizationArgs) -> Result<()> {
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: EvmSpotPolicyV1 = read_json(&args.policy)?;
    let owner = parse_address("owner", &args.owner)?;
    validate_evm_spot_manifest_entry(&entry, &policy, owner)?;
    let certificates = load_evm_spot_certificates(
        &args.checkpoint_certificates,
        &policy,
        &context.pftl_genesis_hash,
    )?;
    let observed_at_pftl_height = common_pftl_observation_height(&certificates)?;
    let verify_context = EvmSpotVerifyContextV1 {
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
        observed_at_pftl_height,
        expected_evidence_commitment: "",
    };
    let certificate_refs = certificates.iter().collect::<Vec<_>>();
    let statement = evm_spot_owner_authorization_statement_v1(
        &policy,
        owner,
        &certificate_refs,
        &verify_context,
    )
    .map_err(|error| anyhow::anyhow!("EVM spot owner statement failed: {error:?}"))?;
    write_new(&args.output, &statement)?;
    print_report(
        "postfiat.reserve_evm_spot_owner_authorization.v1",
        &args.output,
        &statement,
    )
}

fn evm_spot_collect(args: EvmSpotCollectArgs) -> Result<()> {
    anyhow::ensure!(
        args.total_liabilities <= args.gross_assets,
        "total liabilities exceed gross assets"
    );
    validate_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: EvmSpotPolicyV1 = read_json(&args.policy)?;
    let owner = parse_address("owner", &args.owner)?;
    validate_evm_spot_manifest_entry(&entry, &policy, owner)?;
    let certificates = load_evm_spot_certificates(
        &args.checkpoint_certificates,
        &policy,
        &context.pftl_genesis_hash,
    )?;
    let observed_at_pftl_height = common_pftl_observation_height(&certificates)?;
    anyhow::ensure!(
        observed_at_pftl_height >= context.observation_not_before
            && observed_at_pftl_height <= context.observation_not_after,
        "EVM spot checkpoint PFTL height is outside the observation interval"
    );
    let rpc_map: EvmSpotRpcMapV1 = read_json(&args.rpc_map)?;
    anyhow::ensure!(
        rpc_map.schema == "postfiat.reserve_evm_spot_rpc_map.v1",
        "EVM spot RPC map schema mismatch"
    );
    let expected_domains = policy
        .chains
        .iter()
        .map(|chain| chain.source_domain.clone())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        rpc_map.sources.len() == expected_domains.len()
            && expected_domains
                .iter()
                .all(|domain| rpc_map.sources.contains_key(domain)),
        "EVM spot RPC map must contain exactly the governed source domains"
    );
    let ownership_signature = decode_hex("ownership_signature", &args.ownership_signature, 65)?;
    let valuation_evidence: SourceEvidenceV1 = read_json(&args.valuation_evidence)?;
    anyhow::ensure!(
        valuation_evidence.class() == entry.valuation_evidence_class,
        "valuation evidence trust class does not match manifest"
    );

    let mut chains = Vec::with_capacity(policy.chains.len());
    for (chain_policy, certificate) in policy.chains.iter().zip(certificates) {
        let raw_url = rpc_map
            .sources
            .get(&chain_policy.source_domain)
            .context("governed EVM spot RPC source is absent")?;
        let rpc_url = validate_rpc_url(raw_url)?;
        chains.push(collect_evm_spot_chain(
            chain_policy,
            certificate,
            owner,
            &rpc_url,
        )?);
    }
    let proof = EvmSpotQuantityProofV1 {
        policy,
        owner,
        ownership_signature,
        chains,
    };
    let evidence_commitment = proof
        .evidence_commitment()
        .map_err(|error| anyhow::anyhow!("EVM spot evidence commitment failed: {error:?}"))?;
    let verify_context = EvmSpotVerifyContextV1 {
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
        observed_at_pftl_height,
        expected_evidence_commitment: &evidence_commitment,
    };
    let verified = verify_evm_spot_quantity_proof_v1(&proof, &verify_context)
        .map_err(|error| anyhow::anyhow!("EVM spot quantity proof failed: {error:?}"))?;
    let observation = SourceObservationV1 {
        source_id: entry.source_id.clone(),
        observed_at_block: observed_at_pftl_height,
        gross_assets: args.gross_assets,
        total_liabilities: args.total_liabilities,
        quantity_evidence: SourceEvidenceV1::EvmSpotQuantity {
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
    write_new(&args.output, &serde_json::to_vec_pretty(&observation)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_evm_spot_collection.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "pftl_observation_height": observed_at_pftl_height,
            "chain_count": proof_chain_count(&observation.quantity_evidence),
            "position_count": verified.rows.len(),
            "minimum_source_timestamp_ms": verified.minimum_source_timestamp_ms,
            "maximum_source_timestamp_ms": verified.maximum_source_timestamp_ms,
            "quantity_evidence_commitment": evidence_commitment,
            "quantity_trust": "cryptographic_bft_checkpoint_mpt",
            "valuation_trust": format!("{:?}", entry.valuation_evidence_class).to_lowercase(),
            "next_required_check": "attach complete valuation evidence, then run observe for the full manifest",
        }))?
    );
    Ok(())
}

fn proof_chain_count(evidence: &SourceEvidenceV1) -> usize {
    match evidence {
        SourceEvidenceV1::EvmSpotQuantity { proof, .. } => proof.chains.len(),
        _ => 0,
    }
}

fn validate_evm_spot_manifest_entry(
    entry: &SourceManifestEntryV1,
    policy: &EvmSpotPolicyV1,
    owner: Address,
) -> Result<()> {
    policy
        .validate()
        .map_err(|error| anyhow::anyhow!("EVM spot policy is invalid: {error:?}"))?;
    anyhow::ensure!(
        entry.adapter_kind == EVM_SPOT_ADAPTER_KIND_V1 && entry.adapter_schema_version == 1,
        "source does not use {EVM_SPOT_ADAPTER_KIND_V1} schema 1"
    );
    anyhow::ensure!(
        entry.quantity_evidence_class == TrustClassV1::Cryptographic,
        "EVM spot quantity source must be classified cryptographic"
    );
    anyhow::ensure!(
        entry.source_domain == policy.aggregate_source_domain
            && entry.asset_or_position_id == policy.aggregate_position_id,
        "EVM spot policy identity does not match manifest"
    );
    anyhow::ensure!(
        entry.reserve_owner_commitment == evm_spot_owner_commitment(owner),
        "EVM spot owner does not match manifest reserve_owner_commitment"
    );
    anyhow::ensure!(
        entry.quantity_verifier_commitment
            == policy
                .commitment()
                .map_err(|error| anyhow::anyhow!("EVM spot policy commitment failed: {error:?}"))?,
        "EVM spot policy does not match manifest quantity verifier"
    );
    Ok(())
}

fn load_evm_spot_certificates(
    paths: &[PathBuf],
    policy: &EvmSpotPolicyV1,
    pftl_genesis_hash: &str,
) -> Result<Vec<BftSourceCheckpointCertificateV1>> {
    anyhow::ensure!(
        paths.len() == policy.chains.len(),
        "one checkpoint certificate is required for every governed EVM chain"
    );
    let mut by_domain = BTreeMap::new();
    for path in paths {
        let certificate: BftSourceCheckpointCertificateV1 = read_json(path)?;
        certificate.verify().map_err(anyhow::Error::msg)?;
        let checkpoint = &certificate.checkpoint;
        anyhow::ensure!(
            checkpoint.pftl_genesis_hash == pftl_genesis_hash
                && checkpoint.checkpoint_kind == EVM_SPOT_CHECKPOINT_KIND_V1,
            "EVM spot checkpoint has the wrong chain or kind"
        );
        anyhow::ensure!(
            by_domain
                .insert(checkpoint.source_domain.clone(), certificate)
                .is_none(),
            "duplicate EVM spot checkpoint source domain"
        );
    }
    policy
        .chains
        .iter()
        .map(|chain| {
            let certificate = by_domain
                .remove(&chain.source_domain)
                .with_context(|| format!("missing checkpoint for {}", chain.source_domain))?;
            anyhow::ensure!(
                certificate.checkpoint.committee_root == chain.committee_root,
                "EVM spot checkpoint committee does not match policy for {}",
                chain.source_domain
            );
            Ok(certificate)
        })
        .collect()
}

fn common_pftl_observation_height(
    certificates: &[BftSourceCheckpointCertificateV1],
) -> Result<u64> {
    let first = certificates
        .first()
        .context("EVM spot checkpoint set is empty")?
        .checkpoint
        .pftl_observation_height;
    anyhow::ensure!(
        certificates
            .iter()
            .all(|certificate| certificate.checkpoint.pftl_observation_height == first),
        "EVM spot checkpoints do not share one PFTL observation height"
    );
    Ok(first)
}

fn collect_evm_spot_chain(
    policy: &reserve_proof_types::evm_spot::EvmSpotChainPolicyV1,
    certificate: BftSourceCheckpointCertificateV1,
    owner: Address,
    rpc_url: &Url,
) -> Result<EvmSpotChainProofV1> {
    let client = Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()?;
    let checkpoint = &certificate.checkpoint;
    let chain_id: String = rpc_call(&client, rpc_url, "eth_chainId", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_u64_quantity("eth_chainId", &chain_id)? == policy.chain_id,
        "EVM RPC chain ID does not match policy"
    );
    let block_tag = format!("0x{:x}", checkpoint.source_height);
    let block: RpcBlock = rpc_call(
        &client,
        rpc_url,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    let timestamp_ms = parse_u64_quantity("block.timestamp", &block.timestamp)?
        .checked_mul(1_000)
        .context("EVM block timestamp milliseconds overflow")?;
    anyhow::ensure!(
        parse_u64_quantity("block.number", &block.number)? == checkpoint.source_height
            && parse_b256("block.hash", &block.hash)? == checkpoint.source_block_hash
            && parse_b256("block.stateRoot", &block.state_root)?
                == checkpoint.source_state_commitment
            && timestamp_ms == checkpoint.source_timestamp_ms,
        "EVM RPC block does not match the certified checkpoint"
    );
    let latest: String = rpc_call(&client, rpc_url, "eth_blockNumber", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_u64_quantity("latest block", &latest)? >= checkpoint.observed_source_head,
        "EVM RPC head is behind the certified observation head"
    );

    let native_rpc: RpcAccountProof = rpc_call(
        &client,
        rpc_url,
        "eth_getProof",
        serde_json::json!([format!("{owner:#x}"), Vec::<String>::new(), block_tag]),
    )?;
    anyhow::ensure!(
        parse_address("native eth_getProof.address", &native_rpc.address)? == owner
            && native_rpc.storage_proof.is_empty(),
        "native eth_getProof returned a substituted account or storage proof"
    );
    let native_account = rpc_account_proof("native account", native_rpc)?;

    let mut tokens = Vec::with_capacity(policy.tokens.len());
    for token in &policy.tokens {
        let storage_key = spot_balance_slot(owner, token.balance_slot_index);
        let rpc_proof: RpcAccountProof = rpc_call(
            &client,
            rpc_url,
            "eth_getProof",
            serde_json::json!([
                format!("{:#x}", token.token),
                [format!("{storage_key:#x}")],
                format!("0x{:x}", checkpoint.source_height)
            ]),
        )?;
        anyhow::ensure!(
            parse_address("token eth_getProof.address", &rpc_proof.address)? == token.token
                && rpc_proof.storage_proof.len() == 1,
            "token eth_getProof returned a substituted account or storage set"
        );
        let storage = &rpc_proof.storage_proof[0];
        anyhow::ensure!(
            parse_b256("storageProof.key", &storage.key)? == storage_key,
            "token storage proof key does not match the governed balance slot"
        );
        let balance = EvmStorageProofV1 {
            key: storage_key,
            value: parse_u256("storageProof.value", &storage.value)?,
            proof: decode_proof_nodes("storageProof", &storage.proof)?,
        };
        tokens.push(EvmSpotTokenProofV1 {
            position_id: token.position_id.clone(),
            token_account: rpc_account_proof("token account", rpc_proof)?,
            balance,
        });
    }
    Ok(EvmSpotChainProofV1 {
        checkpoint_certificate: certificate,
        native_account,
        tokens,
    })
}

fn rpc_account_proof(label: &str, proof: RpcAccountProof) -> Result<EvmAccountProofV1> {
    Ok(EvmAccountProofV1 {
        address: parse_address(&format!("{label}.address"), &proof.address)?,
        nonce: parse_u64_quantity(&format!("{label}.nonce"), &proof.nonce)?,
        balance: parse_u256(&format!("{label}.balance"), &proof.balance)?,
        storage_root: parse_b256(&format!("{label}.storageHash"), &proof.storage_hash)?,
        code_hash: parse_b256(&format!("{label}.codeHash"), &proof.code_hash)?,
        proof: decode_proof_nodes(&format!("{label}.accountProof"), &proof.account_proof)?,
    })
}

fn collect(args: CollectArgs) -> Result<()> {
    anyhow::ensure!(
        args.total_liabilities <= args.gross_assets,
        "total liabilities exceed gross assets"
    );
    validate_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let certificate: EvmStateCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    certificate.verify().map_err(anyhow::Error::msg)?;
    let checkpoint = certificate.checkpoint.clone();
    anyhow::ensure!(
        checkpoint.pftl_genesis_hash == context.pftl_genesis_hash
            && checkpoint.source_domain == entry.source_domain,
        "checkpoint context does not match manifest/context"
    );
    let committee_root = certificate.committee.root().map_err(anyhow::Error::msg)?;
    let owner = parse_address("owner", &args.owner)?;
    let token = parse_address("token", &args.token)?;
    validate_evm_manifest_entry(&entry, owner, token, &committee_root)?;
    let slot_index = parse_u256("balance_slot_index", &args.balance_slot_index)?;
    let ownership_signature = decode_hex("ownership_signature", &args.ownership_signature, 65)?;
    let valuation_evidence: SourceEvidenceV1 = read_json(&args.valuation_evidence)?;
    anyhow::ensure!(
        valuation_evidence.class() == entry.valuation_evidence_class,
        "valuation evidence trust class does not match manifest"
    );

    let rpc_url = validate_rpc_url(&args.ethereum_rpc_url)?;
    let client = Client::builder()
        .timeout(RPC_TIMEOUT)
        .redirect(Policy::none())
        .build()?;
    let block_tag = format!("0x{:x}", checkpoint.block_number);
    let block: RpcBlock = rpc_call(
        &client,
        &rpc_url,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    anyhow::ensure!(
        parse_u64_quantity("block.number", &block.number)? == checkpoint.block_number,
        "Ethereum RPC returned the wrong block number"
    );
    anyhow::ensure!(
        parse_b256("block.hash", &block.hash)? == checkpoint.block_hash,
        "Ethereum RPC block hash does not match checkpoint"
    );
    anyhow::ensure!(
        parse_b256("block.stateRoot", &block.state_root)? == checkpoint.state_root,
        "Ethereum RPC state root does not match checkpoint"
    );
    let latest: String = rpc_call(&client, &rpc_url, "eth_blockNumber", serde_json::json!([]))?;
    anyhow::ensure!(
        parse_u64_quantity("latest block", &latest)? >= checkpoint.observed_head_number,
        "Ethereum RPC head is behind the signed checkpoint head"
    );

    let storage_key = erc20_balance_slot(owner, slot_index);
    let rpc_proof: RpcAccountProof = rpc_call(
        &client,
        &rpc_url,
        "eth_getProof",
        serde_json::json!([
            format!("{token:#x}"),
            [format!("{storage_key:#x}")],
            format!("0x{:x}", checkpoint.block_number)
        ]),
    )?;
    anyhow::ensure!(
        parse_address("eth_getProof.address", &rpc_proof.address)? == token,
        "eth_getProof returned the wrong account"
    );
    anyhow::ensure!(
        rpc_proof.storage_proof.len() == 1,
        "eth_getProof must return exactly one storage proof"
    );
    let rpc_storage = &rpc_proof.storage_proof[0];
    let account_proof = decode_proof_nodes("accountProof", &rpc_proof.account_proof)?;
    let storage_proof = decode_proof_nodes("storageProof", &rpc_storage.proof)?;
    let proof = EvmErc20BalanceProofV1 {
        checkpoint_certificate: certificate,
        owner,
        ownership_signature,
        token,
        balance_slot_index: slot_index,
        token_account: EvmAccountProofV1 {
            address: token,
            nonce: parse_u64_quantity("eth_getProof.nonce", &rpc_proof.nonce)?,
            balance: parse_u256("eth_getProof.balance", &rpc_proof.balance)?,
            storage_root: parse_b256("eth_getProof.storageHash", &rpc_proof.storage_hash)?,
            code_hash: parse_b256("eth_getProof.codeHash", &rpc_proof.code_hash)?,
            proof: account_proof,
        },
        balance: EvmStorageProofV1 {
            key: parse_b256("storageProof.key", &rpc_storage.key)?,
            value: parse_u256("storageProof.value", &rpc_storage.value)?,
            proof: storage_proof,
        },
    };
    let evidence_commitment = proof.commitment().map_err(anyhow::Error::msg)?;
    proof
        .verify(
            &context.pftl_genesis_hash,
            &context.nav_asset_id,
            &context.proof_profile_id,
            &context.valuation_policy_hash,
            &context.source_manifest_hash,
            &entry.source_id,
            &entry.source_domain,
            &entry.asset_or_position_id,
            &entry.reserve_owner_commitment,
            &entry.quantity_verifier_commitment,
            checkpoint.pftl_observation_height,
            &evidence_commitment,
        )
        .map_err(anyhow::Error::msg)?;
    let token_balance = proof.balance.value;
    let observation = SourceObservationV1 {
        source_id: entry.source_id,
        observed_at_block: checkpoint.pftl_observation_height,
        gross_assets: args.gross_assets,
        total_liabilities: args.total_liabilities,
        quantity_evidence: SourceEvidenceV1::EvmErc20BftCheckpointMpt {
            evidence_commitment: evidence_commitment.clone(),
            proof: Box::new(proof),
        },
        valuation_evidence,
        disclosure_commitment: args.disclosure_commitment,
    };
    write_new(&args.output, &serde_json::to_vec_pretty(&observation)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_evm_erc20_collection.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "ethereum_block_number": checkpoint.block_number,
            "pftl_observation_height": checkpoint.pftl_observation_height,
            "token_balance_atoms": token_balance.to_string(),
            "quantity_evidence_commitment": evidence_commitment,
            "quantity_trust": "cryptographic_bft_checkpoint_mpt",
            "valuation_trust": format!("{:?}", entry.valuation_evidence_class).to_lowercase(),
            "next_required_check": "postfiat-reserve-proof observe validates the valuation evidence and complete manifest",
        }))?
    );
    Ok(())
}

pub(crate) fn load_source(
    manifest_path: &Path,
    context_path: &Path,
    source_id: &str,
) -> Result<(
    SourceManifestV1,
    ReserveProofContextV1,
    SourceManifestEntryV1,
)> {
    let manifest: SourceManifestV1 = read_json(manifest_path)?;
    manifest.validate().map_err(anyhow::Error::msg)?;
    let context: ReserveProofContextV1 = read_json(context_path)?;
    anyhow::ensure!(
        manifest.hash().map_err(anyhow::Error::msg)? == context.source_manifest_hash,
        "context source_manifest_hash does not match manifest"
    );
    let entry = manifest
        .sources
        .iter()
        .find(|entry| entry.source_id == source_id)
        .cloned()
        .with_context(|| format!("source_id is not present in manifest: {source_id}"))?;
    Ok((manifest, context, entry))
}

fn validate_evm_manifest_entry(
    entry: &SourceManifestEntryV1,
    owner: Address,
    token: Address,
    committee_root: &str,
) -> Result<()> {
    anyhow::ensure!(
        entry.adapter_kind == EVM_ERC20_ADAPTER_KIND_V1,
        "source does not use {EVM_ERC20_ADAPTER_KIND_V1}"
    );
    anyhow::ensure!(
        entry.quantity_evidence_class == TrustClassV1::Cryptographic,
        "EVM MPT quantity source must be classified cryptographic"
    );
    anyhow::ensure!(
        entry.reserve_owner_commitment == evm_owner_commitment(owner),
        "owner does not match manifest reserve_owner_commitment"
    );
    anyhow::ensure!(
        entry.asset_or_position_id == format!("erc20:0x{}", hex::encode(token.as_slice())),
        "token does not match manifest asset_or_position_id"
    );
    anyhow::ensure!(
        entry.quantity_verifier_commitment == committee_root,
        "committee root does not match manifest quantity verifier"
    );
    Ok(())
}

pub(crate) fn validate_rpc_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).context("parse Ethereum RPC URL")?;
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Ethereum RPC credentials must not be embedded in the URL"
    );
    let loopback = url.host_str().is_some_and(|host| {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        host.eq_ignore_ascii_case("localhost")
            || IpAddr::from_str(host).is_ok_and(|address| address.is_loopback())
    });
    anyhow::ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && loopback),
        "Ethereum RPC must use HTTPS, except loopback HTTP for development"
    );
    Ok(url)
}

pub(crate) fn rpc_call<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &Url,
    method: &str,
    params: serde_json::Value,
) -> Result<T> {
    let mut response = client
        .post(url.clone())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }))
        .send()
        .with_context(|| format!("JSON-RPC {method}"))?;
    anyhow::ensure!(
        response.status().is_success(),
        "JSON-RPC {method} returned HTTP {}",
        response.status()
    );
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take((MAX_RPC_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= MAX_RPC_RESPONSE_BYTES,
        "JSON-RPC {method} response exceeds {MAX_RPC_RESPONSE_BYTES} bytes"
    );
    let envelope: RpcEnvelope<T> =
        serde_json::from_slice(&bytes).with_context(|| format!("decode JSON-RPC {method}"))?;
    anyhow::ensure!(
        envelope.jsonrpc == "2.0" && envelope.id == serde_json::json!(1),
        "JSON-RPC {method} returned a mismatched envelope"
    );
    if let Some(error) = envelope.error {
        bail!(
            "JSON-RPC {method} failed with {}: {}",
            error.code,
            error.message
        );
    }
    envelope
        .result
        .with_context(|| format!("JSON-RPC {method} omitted result"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcEnvelope<T> {
    jsonrpc: String,
    id: serde_json::Value,
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcError {
    code: i64,
    message: String,
    #[serde(default)]
    #[serde(rename = "data")]
    _data: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RpcBlock {
    number: String,
    hash: String,
    state_root: String,
    timestamp: String,
    #[serde(flatten)]
    _extra: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RpcAccountProof {
    address: String,
    account_proof: Vec<String>,
    balance: String,
    code_hash: String,
    nonce: String,
    storage_hash: String,
    storage_proof: Vec<RpcStorageProof>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcStorageProof {
    key: String,
    value: String,
    proof: Vec<String>,
}

pub(crate) fn parse_address(label: &str, value: &str) -> Result<Address> {
    Address::from_str(value).with_context(|| format!("{label} is not a canonical EVM address"))
}

pub(crate) fn parse_b256(label: &str, value: &str) -> Result<B256> {
    B256::from_str(value).with_context(|| format!("{label} is not a 32-byte hex value"))
}

fn parse_u256(label: &str, value: &str) -> Result<U256> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    anyhow::ensure!(!digits.is_empty(), "{label} is empty");
    U256::from_str_radix(digits, 16).with_context(|| format!("{label} is not a hex quantity"))
}

pub(crate) fn parse_u64_quantity(label: &str, value: &str) -> Result<u64> {
    let parsed = parse_u256(label, value)?;
    u64::try_from(parsed).with_context(|| format!("{label} exceeds u64"))
}

pub(crate) fn decode_hex(label: &str, value: &str, expected_bytes: usize) -> Result<Vec<u8>> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    anyhow::ensure!(
        digits.len() == expected_bytes.saturating_mul(2),
        "{label} must be exactly {expected_bytes} bytes"
    );
    hex::decode(digits).with_context(|| format!("{label} is not hex"))
}

fn decode_proof_nodes(label: &str, values: &[String]) -> Result<Vec<Vec<u8>>> {
    anyhow::ensure!(!values.is_empty(), "{label} is empty");
    let mut total = 0usize;
    let mut nodes = Vec::with_capacity(values.len());
    for value in values {
        let digits = value.strip_prefix("0x").unwrap_or(value);
        let node = hex::decode(digits).with_context(|| format!("{label} contains non-hex data"))?;
        total = total
            .checked_add(node.len())
            .context("proof size overflow")?;
        anyhow::ensure!(
            total <= MAX_EVM_PROOF_TOTAL_BYTES,
            "{label} exceeds {MAX_EVM_PROOF_TOTAL_BYTES} bytes"
        );
        nodes.push(node);
    }
    Ok(nodes)
}

fn validate_hex(label: &str, value: &str, bytes: usize) -> Result<()> {
    anyhow::ensure!(
        value.len() == bytes.saturating_mul(2)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{label} must be canonical lowercase hex"
    );
    Ok(())
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file() && metadata.len() <= MAX_WITNESS_BYTES as u64,
        "input must be a regular file no larger than {MAX_WITNESS_BYTES} bytes: {}",
        path.display()
    );
    serde_json::from_slice(&fs::read(path)?).with_context(|| format!("decode {}", path.display()))
}

pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    anyhow::ensure!(!path.exists(), "refusing to overwrite {}", path.display());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn print_report(schema: &str, output: &Path, statement: &[u8]) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": schema,
            "output": output,
            "statement_bytes": statement.len(),
            "statement_hex": hex::encode(statement),
        }))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_url_policy_is_fail_closed() {
        assert!(validate_rpc_url("https://rpc.example").is_ok());
        assert!(validate_rpc_url("http://127.0.0.1:8545").is_ok());
        assert!(validate_rpc_url("http://[::1]:8545").is_ok());
        assert!(validate_rpc_url("http://rpc.example").is_err());
        assert!(validate_rpc_url("https://user:secret@rpc.example").is_err());
        assert!(validate_rpc_url("file:///tmp/socket").is_err());
    }

    #[test]
    fn quantity_and_proof_parsers_enforce_bounds() {
        assert_eq!(parse_u64_quantity("height", "0x2a").unwrap(), 42);
        assert!(parse_u64_quantity("height", &format!("0x1{}", "0".repeat(16))).is_err());
        assert!(decode_hex("signature", "0x11", 65).is_err());
        let oversized = format!("0x{}", "11".repeat(MAX_EVM_PROOF_TOTAL_BYTES + 1));
        assert!(decode_proof_nodes("proof", &[oversized]).is_err());
    }
}
