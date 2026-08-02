use std::{path::PathBuf, str::FromStr, time::Duration};

use alloy_primitives::B256;
use anyhow::{anyhow, bail, Context, Result};
use clap::Subcommand;
use monero::{
    consensus::encode::{deserialize, serialize},
    cryptonote::hash::Hashable,
    util::address::Address,
    Network,
};
use reqwest::{blocking::Client, redirect::Policy, Url};
use reserve_proof_types::{
    bft_checkpoint::{
        BftCheckpointCommitteeV1, BftSourceCheckpointCertificateV1, BftSourceCheckpointV1,
    },
    monero_reserve::{
        fuzz_monero_transaction_bytes, monero_reserve_challenge_v1,
        monero_reserve_owner_commitment, monero_status_commitment, verify_monero_reserve_proof_v1,
        verify_xmr_reserve_witness, MoneroKeyImageStatusV1, MoneroReservePolicyV1,
        MoneroReserveProofV1, MoneroReserveVerifyContextV1, XmrBlockAnchor, XmrBlockHeaderLink,
        XmrReserveOutputWitness, XmrReserveWitness, XmrSignature, XmrSubaddrSpendKeyProof,
        XmrTxTreeProof, MONERO_CHECKPOINT_KIND_V1, MONERO_RESERVE_ADAPTER_KIND_V1,
        XMR_RESERVE_MAX_OUTPUTS,
    },
    verify_observation_evidence, EvidenceDimensionV1, ReserveProofContextV1, SourceEvidenceV1,
    SourceManifestEntryV1, SourceObservationV1, TrustClassV1,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::checkpoint_signing::{maybe_sign_reproduced_checkpoint, CheckpointSigningArgs};
use crate::evm_adapter::{load_source, read_json, validate_rpc_url, write_new};

const RESERVE_PROOF_V2_PREFIX: &str = "ReserveProofV2";
const RPC_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_RPC_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESERVE_PROOF_TEXT_BYTES: usize = 128 * 1024;

#[derive(Debug, Subcommand)]
pub enum MoneroCommand {
    /// Emit the exact context-bound message that an external Monero wallet
    /// must use when creating its ReserveProofV2.
    Challenge {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        context: PathBuf,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        pftl_observation_height: u64,
        #[arg(long)]
        output: PathBuf,
    },
    /// Parse a wallet-created ReserveProofV2, collect exact transactions and
    /// block/header proofs, check key-image status, and emit a checkpoint
    /// candidate for independent validator signing.
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
        reserve_proof: PathBuf,
        #[arg(long)]
        committee: PathBuf,
        #[arg(long)]
        pftl_observation_height: u64,
        /// Common daemon head pinned by the checkpoint coordinator. Every
        /// validator independently requires its live head to be at least this
        /// value before signing.
        #[arg(long)]
        observed_source_head: u64,
        #[arg(long)]
        minimum_depth: u32,
        #[arg(long)]
        daemon_url: String,
        #[arg(long)]
        prepared_output: PathBuf,
        #[arg(long)]
        checkpoint_output: PathBuf,
        #[command(flatten)]
        signing: CheckpointSigningArgs,
    },
    /// Attach the certified current key-image-status checkpoint and emit a
    /// quantity-verified draft for a separate cryptographic valuation adapter.
    /// The draft cannot pass valuation checks until that adapter replaces its
    /// deliberately invalid placeholder.
    CollectQuantity {
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
        disclosure_commitment: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Attach the independently assembled checkpoint certificate, verify the
    /// complete quantity and valuation evidence, and write the observation.
    Collect {
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

pub fn run(command: MoneroCommand) -> Result<()> {
    match command {
        MoneroCommand::Challenge {
            manifest,
            context,
            source_id,
            policy,
            pftl_observation_height,
            output,
        } => challenge(
            manifest,
            context,
            source_id,
            policy,
            pftl_observation_height,
            output,
        ),
        MoneroCommand::Prepare {
            manifest,
            context,
            source_id,
            policy,
            reserve_proof,
            committee,
            pftl_observation_height,
            observed_source_head,
            minimum_depth,
            daemon_url,
            prepared_output,
            checkpoint_output,
            signing,
        } => prepare(PrepareArgs {
            manifest,
            context,
            source_id,
            policy,
            reserve_proof,
            committee,
            pftl_observation_height,
            observed_source_head,
            minimum_depth,
            daemon_url,
            prepared_output,
            checkpoint_output,
            signing,
        }),
        MoneroCommand::CollectQuantity {
            manifest,
            context,
            source_id,
            prepared,
            checkpoint_certificate,
            disclosure_commitment,
            output,
        } => collect_quantity(CollectQuantityArgs {
            manifest,
            context,
            source_id,
            prepared,
            checkpoint_certificate,
            disclosure_commitment,
            output,
        }),
        MoneroCommand::Collect {
            manifest,
            context,
            source_id,
            prepared,
            checkpoint_certificate,
            valuation_evidence,
            gross_assets,
            total_liabilities,
            disclosure_commitment,
            output,
        } => collect(CollectArgs {
            manifest,
            context,
            source_id,
            prepared,
            checkpoint_certificate,
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
struct MoneroChallengeV1 {
    schema: &'static str,
    source_id: String,
    pftl_observation_height: u64,
    message: String,
    next_required_check: &'static str,
}

fn challenge(
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy_path: PathBuf,
    observed_at: u64,
    output: PathBuf,
) -> Result<()> {
    let (_, context, entry) = load_source(&manifest, &context, &source_id)?;
    let policy: MoneroReservePolicyV1 = read_json(&policy_path)?;
    validate_manifest_entry(&entry, &policy)?;
    validate_observation_height(&context, observed_at)?;
    let verify_context = verification_context(&context, &entry, observed_at, "");
    let message = monero_reserve_challenge_v1(&policy, &verify_context)
        .map_err(|error| anyhow!("Monero challenge failed: {error}"))?;
    let artifact = MoneroChallengeV1 {
        schema: "postfiat.reserve_monero_challenge.v1",
        source_id,
        pftl_observation_height: observed_at,
        message,
        next_required_check: "create a ReserveProofV2 for this exact message with an external Monero wallet; no wallet seed or view key enters this CLI",
    };
    write_new(&output, &serde_json::to_vec_pretty(&artifact)?)?;
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    Ok(())
}

struct PrepareArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    policy: PathBuf,
    reserve_proof: PathBuf,
    committee: PathBuf,
    pftl_observation_height: u64,
    observed_source_head: u64,
    minimum_depth: u32,
    daemon_url: String,
    prepared_output: PathBuf,
    checkpoint_output: PathBuf,
    signing: CheckpointSigningArgs,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedMoneroReserveV1 {
    schema: String,
    policy: MoneroReservePolicyV1,
    reserve: XmrReserveWitness,
    key_image_statuses: Vec<MoneroKeyImageStatusV1>,
    checkpoint: BftSourceCheckpointV1,
}

fn prepare(args: PrepareArgs) -> Result<()> {
    anyhow::ensure!(
        !args.prepared_output.exists() && !args.checkpoint_output.exists(),
        "refusing to overwrite Monero prepared or checkpoint output"
    );
    anyhow::ensure!(
        args.observed_source_head > 0 && args.minimum_depth > 0,
        "observation head and minimum depth must be nonzero"
    );
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let policy: MoneroReservePolicyV1 = read_json(&args.policy)?;
    validate_manifest_entry(&entry, &policy)?;
    validate_observation_height(&context, args.pftl_observation_height)?;
    let committee: BftCheckpointCommitteeV1 = read_json(&args.committee)?;
    let committee_root = committee.root().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        committee_root == policy.checkpoint_committee_root,
        "Monero committee root does not match policy"
    );
    let input: ReserveProofJson = read_json(&args.reserve_proof)?;
    let expected_message = monero_reserve_challenge_v1(
        &policy,
        &verification_context(&context, &entry, args.pftl_observation_height, ""),
    )
    .map_err(|error| anyhow!("Monero challenge failed: {error}"))?;
    anyhow::ensure!(
        input.message == expected_message,
        "ReserveProofV2 message does not match the current NAV/profile/manifest/epoch context"
    );
    let parsed = parse_reserve_proof_v2(&input.signature)?;
    anyhow::ensure!(
        parsed.entries.len() <= usize::from(policy.max_outputs),
        "ReserveProofV2 exceeds policy output bound"
    );
    let address = Address::from_str(&input.address)
        .with_context(|| format!("parse Monero address {}", input.address))?;
    anyhow::ensure!(
        address.network == Network::Mainnet,
        "Monero address is not mainnet"
    );
    anyhow::ensure!(
        B256::from(address.public_spend.to_bytes()) == policy.address_spend_public_key
            && B256::from(address.public_view.to_bytes()) == policy.address_view_public_key,
        "Monero address keys do not match policy"
    );
    let rpc = MoneroRpc::new(&args.daemon_url)?;
    let live_observed_head = rpc.last_block_header()?;
    anyhow::ensure!(
        live_observed_head.height >= args.observed_source_head,
        "live Monero head is behind the pinned observation head"
    );
    let source_height = args
        .observed_source_head
        .checked_sub(u64::from(args.minimum_depth))
        .context("Monero head is below the required confirmation depth")?;
    let source_block = rpc.fetch_block(source_height)?;
    anyhow::ensure!(
        source_block.hash == parse_b256(&rpc.block_header(source_height)?.hash)?,
        "Monero source block and header RPC disagree"
    );

    let mut entries = Vec::with_capacity(parsed.entries.len());
    for parsed_entry in parsed.entries {
        let tx = rpc.fetch_transaction(parsed_entry.txid)?;
        anyhow::ensure!(!tx.in_pool, "reserve transaction is still in the mempool");
        anyhow::ensure!(
            tx.block_height <= source_height,
            "reserve transaction has not reached the certified source block"
        );
        let transaction_bytes = decode_hex_bounded(
            "Monero transaction",
            &tx.as_hex,
            reserve_proof_types::monero_reserve::XMR_RESERVE_MAX_TRANSACTION_BYTES,
        )?;
        let parsed_tx: monero::Transaction =
            deserialize(&transaction_bytes).context("decode Monero transaction")?;
        anyhow::ensure!(
            B256::from(parsed_tx.hash().to_bytes()) == parsed_entry.txid
                && normalize_hex(&tx.tx_hash) == hex::encode(parsed_entry.txid),
            "Monero daemon returned a substituted transaction"
        );
        let output_block = rpc.fetch_block(tx.block_height)?;
        let hashes = output_block.transaction_hashes();
        let index = hashes
            .iter()
            .position(|hash| *hash == parsed_entry.txid)
            .context("reserve transaction is absent from its claimed block")?;
        let branch = monero_tree_branch(&hashes, index)?;
        let anchor = if tx.block_height == source_height {
            anyhow::ensure!(
                policy.allow_pinned_output_blocks,
                "reserve output equals the source height; wait for one more finalized source block"
            );
            XmrBlockAnchor::PinnedOutputBlock {
                pinned_output_block_hash: output_block.hash,
            }
        } else {
            let distance = source_height - tx.block_height;
            if distance <= u64::from(policy.max_header_links_per_output) {
                let mut links = Vec::with_capacity(usize::try_from(distance)?);
                for height in tx.block_height + 1..=source_height {
                    let block = rpc.fetch_block(height)?;
                    links.push(XmrBlockHeaderLink {
                        header_bytes: serialize(&block.block.header),
                        tx_tree_root: B256::from(block.block.tx_root().to_bytes()),
                        tx_count: u64::try_from(block.transaction_hashes().len())?,
                    });
                }
                XmrBlockAnchor::HeaderChain {
                    pinned_head_hash: source_block.hash,
                    links,
                }
            } else if policy.allow_pinned_output_blocks {
                XmrBlockAnchor::PinnedOutputBlock {
                    pinned_output_block_hash: output_block.hash,
                }
            } else {
                bail!(
                    "output at height {} needs {} header links, above policy maximum {}",
                    tx.block_height,
                    distance,
                    policy.max_header_links_per_output
                );
            }
        };
        entries.push(XmrReserveOutputWitness {
            txid: parsed_entry.txid,
            index_in_tx: parsed_entry.index_in_tx,
            shared_secret: parsed_entry.shared_secret,
            key_image: parsed_entry.key_image,
            shared_secret_sig: parsed_entry.shared_secret_sig,
            key_image_sig: parsed_entry.key_image_sig,
            transaction_bytes,
            tx_tree: XmrTxTreeProof {
                tx_tree_root: B256::from(output_block.block.tx_root().to_bytes()),
                tx_count: u64::try_from(hashes.len())?,
                tx_index: u64::try_from(index)?,
                branch,
                output_block_header_bytes: serialize(&output_block.block.header),
            },
            block_anchor: anchor,
        });
    }
    let reserve = XmrReserveWitness {
        address_spend_public_key: policy.address_spend_public_key,
        address_view_public_key: policy.address_view_public_key,
        message: input.message,
        entries,
        subaddr_spendkeys: parsed.subaddr_spendkeys,
    };
    let verified = verify_xmr_reserve_witness(&reserve)
        .map_err(|error| anyhow!("assembled Monero reserve witness failed: {error}"))?;
    if let Some(expected) = input.proven_atomic {
        anyhow::ensure!(
            u128::from(expected) == verified.xmr_atomic,
            "wallet-reported and independently verified Monero quantities disagree"
        );
    }
    let key_image_statuses = rpc.key_image_statuses(&verified.key_images)?;
    anyhow::ensure!(
        key_image_statuses.iter().all(|status| !status.spent),
        "a reserve output is spent at the observed Monero head"
    );
    let source_state_commitment = monero_status_commitment(
        source_block.hash,
        &verified.output_block_hashes,
        &key_image_statuses,
    )
    .map_err(|error| anyhow!("Monero status commitment failed: {error}"))?;
    let checkpoint = BftSourceCheckpointV1 {
        pftl_genesis_hash: context.pftl_genesis_hash.clone(),
        checkpoint_kind: MONERO_CHECKPOINT_KIND_V1.to_string(),
        source_domain: policy.source_domain.clone(),
        source_height,
        source_timestamp_ms: source_block
            .timestamp
            .checked_mul(1_000)
            .context("Monero source timestamp overflow")?,
        source_block_hash: source_block.hash,
        source_state_commitment,
        observed_source_head: args.observed_source_head,
        minimum_depth: args.minimum_depth,
        pftl_observation_height: args.pftl_observation_height,
        committee_epoch: committee.epoch,
        committee_root,
    };
    checkpoint.canonical_bytes().map_err(anyhow::Error::msg)?;
    maybe_sign_reproduced_checkpoint(&checkpoint, &committee, &args.signing)?;
    let prepared = PreparedMoneroReserveV1 {
        schema: "postfiat.reserve_monero_prepared.v1".to_string(),
        policy,
        reserve,
        key_image_statuses,
        checkpoint: checkpoint.clone(),
    };
    write_new(
        &args.prepared_output,
        &serde_json::to_vec_pretty(&prepared)?,
    )?;
    write_new(
        &args.checkpoint_output,
        &serde_json::to_vec_pretty(&checkpoint)?,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema": "postfiat.reserve_monero_prepare_report.v1",
            "prepared_output": args.prepared_output,
            "checkpoint_output": args.checkpoint_output,
            "outputs": prepared.reserve.entries.len(),
            "xmr_atomic": verified.xmr_atomic.to_string(),
            "source_height": checkpoint.source_height,
            "source_hash": checkpoint.source_block_hash,
            "observed_source_head": checkpoint.observed_source_head,
            "live_source_head": live_observed_head.height,
            "minimum_depth": checkpoint.minimum_depth,
            "next_required_check": "each checkpoint validator independently reproduces the source block, output anchors, and key-image statuses before signing",
        }))?
    );
    Ok(())
}

struct CollectArgs {
    manifest: PathBuf,
    context: PathBuf,
    source_id: String,
    prepared: PathBuf,
    checkpoint_certificate: PathBuf,
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
    prepared: PathBuf,
    checkpoint_certificate: PathBuf,
    disclosure_commitment: String,
    output: PathBuf,
}

fn collect_quantity(args: CollectQuantityArgs) -> Result<()> {
    validate_lower_hex("disclosure_commitment", &args.disclosure_commitment, 48)?;
    let (_, context, entry) = load_source(&args.manifest, &args.context, &args.source_id)?;
    let prepared: PreparedMoneroReserveV1 = read_json(&args.prepared)?;
    anyhow::ensure!(
        prepared.schema == "postfiat.reserve_monero_prepared.v1",
        "unsupported Monero prepared artifact schema"
    );
    validate_manifest_entry(&entry, &prepared.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    anyhow::ensure!(
        certificate.checkpoint == prepared.checkpoint,
        "Monero certificate does not sign the prepared checkpoint"
    );
    let proof = MoneroReserveProofV1 {
        policy: prepared.policy,
        checkpoint_certificate: certificate,
        reserve: prepared.reserve,
        key_image_statuses: prepared.key_image_statuses,
    };
    let evidence_commitment = proof
        .evidence_commitment()
        .map_err(|error| anyhow!("Monero evidence commitment failed: {error}"))?;
    let observed_at = proof
        .checkpoint_certificate
        .checkpoint
        .pftl_observation_height;
    validate_observation_height(&context, observed_at)?;
    let verified = verify_monero_reserve_proof_v1(
        &proof,
        &verification_context(&context, &entry, observed_at, &evidence_commitment),
    )
    .map_err(|error| anyhow!("Monero quantity proof failed: {error}"))?;
    let quantity_evidence = SourceEvidenceV1::MoneroReserve {
        evidence_commitment: evidence_commitment.clone(),
        proof: Box::new(proof),
    };
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
        serde_json::to_string_pretty(&json!({
            "schema": "postfiat.reserve_monero_quantity_draft.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "pftl_observation_height": observation.observed_at_block,
            "xmr_atomic": verified.xmr_atomic.to_string(),
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
    let prepared: PreparedMoneroReserveV1 = read_json(&args.prepared)?;
    anyhow::ensure!(
        prepared.schema == "postfiat.reserve_monero_prepared.v1",
        "unsupported Monero prepared artifact schema"
    );
    validate_manifest_entry(&entry, &prepared.policy)?;
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&args.checkpoint_certificate)?;
    anyhow::ensure!(
        certificate.checkpoint == prepared.checkpoint,
        "Monero certificate does not sign the prepared checkpoint"
    );
    let proof = MoneroReserveProofV1 {
        policy: prepared.policy,
        checkpoint_certificate: certificate,
        reserve: prepared.reserve,
        key_image_statuses: prepared.key_image_statuses,
    };
    let evidence_commitment = proof
        .evidence_commitment()
        .map_err(|error| anyhow!("Monero evidence commitment failed: {error}"))?;
    let observed_at = proof
        .checkpoint_certificate
        .checkpoint
        .pftl_observation_height;
    validate_observation_height(&context, observed_at)?;
    let verified = verify_monero_reserve_proof_v1(
        &proof,
        &verification_context(&context, &entry, observed_at, &evidence_commitment),
    )
    .map_err(|error| anyhow!("Monero quantity proof failed: {error}"))?;
    let valuation_evidence: SourceEvidenceV1 = read_json(&args.valuation_evidence)?;
    let observation = SourceObservationV1 {
        source_id: entry.source_id.clone(),
        observed_at_block: observed_at,
        gross_assets: args.gross_assets,
        total_liabilities: args.total_liabilities,
        quantity_evidence: SourceEvidenceV1::MoneroReserve {
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
            "schema": "postfiat.reserve_monero_collection.v1",
            "output": args.output,
            "source_id": observation.source_id,
            "xmr_atomic": verified.xmr_atomic.to_string(),
            "outputs": verified.output_block_hashes.len(),
            "checkpoint_height": verified.checkpoint_height,
            "checkpoint_hash": verified.checkpoint_hash,
            "quantity_trust": "cryptographic_monero_reserve_bft_checkpoint",
            "valuation_trust": format!("{:?}", entry.valuation_evidence_class).to_lowercase(),
            "evidence_commitment": evidence_commitment,
        }))?
    );
    Ok(())
}

fn validate_manifest_entry(
    entry: &SourceManifestEntryV1,
    policy: &MoneroReservePolicyV1,
) -> Result<()> {
    policy
        .validate()
        .map_err(|error| anyhow!("Monero policy is invalid: {error}"))?;
    anyhow::ensure!(
        policy.source_domain == "monero:mainnet",
        "Monero policy must use mainnet"
    );
    anyhow::ensure!(
        entry.adapter_kind == MONERO_RESERVE_ADAPTER_KIND_V1
            && entry.adapter_schema_version == 1
            && entry.quantity_evidence_class == TrustClassV1::Cryptographic,
        "source does not use the cryptographic Monero reserve adapter schema 1"
    );
    anyhow::ensure!(
        entry.source_domain == policy.source_domain
            && entry.asset_or_position_id == policy.position_id
            && entry.reserve_owner_commitment
                == monero_reserve_owner_commitment(
                    policy.address_spend_public_key,
                    policy.address_view_public_key,
                )
            && entry.quantity_verifier_commitment
                == policy
                    .commitment()
                    .map_err(|error| anyhow!("Monero policy commitment failed: {error}"))?,
        "Monero manifest identity, owner, or policy commitment mismatch"
    );
    Ok(())
}

fn validate_observation_height(context: &ReserveProofContextV1, observed_at: u64) -> Result<()> {
    anyhow::ensure!(
        observed_at >= context.observation_not_before
            && observed_at <= context.observation_not_after,
        "Monero PFTL observation height is outside the context interval"
    );
    Ok(())
}

fn verification_context<'a>(
    context: &'a ReserveProofContextV1,
    entry: &'a SourceManifestEntryV1,
    observed_at: u64,
    evidence_commitment: &'a str,
) -> MoneroReserveVerifyContextV1<'a> {
    MoneroReserveVerifyContextV1 {
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
        observation_epoch: context.observation_epoch,
        observation_not_before: context.observation_not_before,
        observation_not_after: context.observation_not_after,
        observed_at_pftl_height: observed_at,
        expected_evidence_commitment: evidence_commitment,
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReserveProofJson {
    address: String,
    message: String,
    #[serde(alias = "reserve_proof")]
    signature: String,
    #[serde(default)]
    proven_atomic: Option<u64>,
    #[serde(default)]
    #[serde(rename = "block_height")]
    _block_height: Option<u64>,
    #[serde(default)]
    #[serde(rename = "node")]
    _node: Option<String>,
    #[serde(default)]
    #[serde(rename = "node_tip")]
    _node_tip: Option<u64>,
    #[serde(default)]
    #[serde(rename = "proven_xmr")]
    _proven_xmr: Option<String>,
    #[serde(default)]
    #[serde(rename = "restore_height")]
    _restore_height: Option<u64>,
}

struct ParsedReserveProofV2 {
    entries: Vec<ParsedReserveEntry>,
    subaddr_spendkeys: Vec<XmrSubaddrSpendKeyProof>,
}

struct ParsedReserveEntry {
    txid: B256,
    index_in_tx: u64,
    shared_secret: B256,
    key_image: B256,
    shared_secret_sig: XmrSignature,
    key_image_sig: XmrSignature,
}

fn parse_reserve_proof_v2(signature: &str) -> Result<ParsedReserveProofV2> {
    anyhow::ensure!(
        signature.len() <= MAX_RESERVE_PROOF_TEXT_BYTES,
        "ReserveProofV2 text exceeds bound"
    );
    let encoded = signature
        .strip_prefix(RESERVE_PROOF_V2_PREFIX)
        .context("reserve proof is not ReserveProofV2")?;
    let decoded = base58_monero::decode(encoded).context("decode Monero ReserveProofV2 base58")?;
    anyhow::ensure!(
        decoded.len() <= MAX_RESERVE_PROOF_TEXT_BYTES,
        "decoded ReserveProofV2 exceeds bound"
    );
    parse_reserve_proof_v2_bytes(&decoded)
}

fn parse_reserve_proof_v2_bytes(decoded: &[u8]) -> Result<ParsedReserveProofV2> {
    anyhow::ensure!(
        decoded.len() <= MAX_RESERVE_PROOF_TEXT_BYTES,
        "decoded ReserveProofV2 exceeds bound"
    );
    let mut reader = ProofReader::new(decoded);
    let entry_count = usize::try_from(reader.read_varint()?)?;
    anyhow::ensure!(
        entry_count <= XMR_RESERVE_MAX_OUTPUTS,
        "reserve output count exceeds bound"
    );
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        anyhow::ensure!(
            reader.read_varint()? == 0,
            "unsupported reserve entry version"
        );
        entries.push(ParsedReserveEntry {
            txid: B256::from(reader.read_32()?),
            index_in_tx: reader.read_varint()?,
            shared_secret: B256::from(reader.read_32()?),
            key_image: B256::from(reader.read_32()?),
            shared_secret_sig: reader.read_signature()?,
            key_image_sig: reader.read_signature()?,
        });
    }
    let key_count = usize::try_from(reader.read_varint()?)?;
    anyhow::ensure!(
        key_count > 0 && key_count <= XMR_RESERVE_MAX_OUTPUTS + 1,
        "unsupported subaddress spend-key proof count"
    );
    let mut subaddr_spendkeys = Vec::with_capacity(key_count);
    for _ in 0..key_count {
        subaddr_spendkeys.push(XmrSubaddrSpendKeyProof {
            archive_marker: reader.read_varint()?,
            public_key: B256::from(reader.read_32()?),
            signature: reader.read_signature()?,
        });
    }
    reader.finish()?;
    Ok(ParsedReserveProofV2 {
        entries,
        subaddr_spendkeys,
    })
}

struct ProofReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ProofReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn read_varint(&mut self) -> Result<u64> {
        let start = self.offset;
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = self.read_u8()?;
            anyhow::ensure!(shift < 64, "ReserveProofV2 varint overflow");
            if shift == 63 {
                anyhow::ensure!(byte <= 1, "ReserveProofV2 varint overflow");
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                let mut canonical = Vec::new();
                write_varint(value, &mut canonical);
                anyhow::ensure!(
                    self.data[start..self.offset] == canonical,
                    "ReserveProofV2 contains a non-canonical varint"
                );
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn read_u8(&mut self) -> Result<u8> {
        let value = self
            .data
            .get(self.offset)
            .copied()
            .context("ReserveProofV2 ended early")?;
        self.offset += 1;
        Ok(value)
    }

    fn read_32(&mut self) -> Result<[u8; 32]> {
        let end = self
            .offset
            .checked_add(32)
            .context("ReserveProofV2 offset overflow")?;
        let value = self
            .data
            .get(self.offset..end)
            .context("ReserveProofV2 ended early")?;
        self.offset = end;
        value.try_into().context("ReserveProofV2 32-byte field")
    }

    fn read_signature(&mut self) -> Result<XmrSignature> {
        Ok(XmrSignature {
            c: B256::from(self.read_32()?),
            r: B256::from(self.read_32()?),
        })
    }

    fn finish(&self) -> Result<()> {
        anyhow::ensure!(
            self.offset == self.data.len(),
            "ReserveProofV2 has trailing bytes"
        );
        Ok(())
    }
}

fn write_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

#[derive(Debug, Deserialize)]
struct GetTransactionsResponse {
    status: String,
    #[serde(default)]
    missed_tx: Vec<String>,
    txs: Vec<GetTransactionEntry>,
}

#[derive(Debug, Deserialize)]
struct GetTransactionEntry {
    as_hex: String,
    block_height: u64,
    in_pool: bool,
    tx_hash: String,
}

#[derive(Debug, Deserialize)]
struct GetBlockResponse {
    result: GetBlockResult,
}

#[derive(Debug, Deserialize)]
struct GetBlockResult {
    blob: String,
    json: String,
}

#[derive(Debug, Deserialize)]
struct GetBlockJson {
    tx_hashes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GetBlockHeaderResponse {
    result: GetBlockHeaderResult,
}

#[derive(Debug, Deserialize)]
struct GetBlockHeaderResult {
    block_header: BlockHeaderJson,
}

#[derive(Clone, Debug, Deserialize)]
struct BlockHeaderJson {
    hash: String,
    height: u64,
    timestamp: u64,
}

#[derive(Debug, Deserialize)]
struct KeyImageSpentResponse {
    status: String,
    spent_status: Vec<u8>,
}

struct MoneroBlock {
    block: monero::Block,
    hash: B256,
    timestamp: u64,
}

impl MoneroBlock {
    fn transaction_hashes(&self) -> Vec<B256> {
        let mut hashes = Vec::with_capacity(1 + self.block.tx_hashes.len());
        hashes.push(B256::from(self.block.miner_tx.hash().to_bytes()));
        hashes.extend(
            self.block
                .tx_hashes
                .iter()
                .map(|hash| B256::from(hash.to_bytes())),
        );
        hashes
    }
}

struct MoneroRpc {
    client: Client,
    base: Url,
}

impl MoneroRpc {
    fn new(value: &str) -> Result<Self> {
        let base = validate_rpc_url(value)?;
        anyhow::ensure!(
            base.path().is_empty() || base.path() == "/",
            "Monero daemon URL must not contain a path"
        );
        let client = Client::builder()
            .timeout(RPC_TIMEOUT)
            .redirect(Policy::none())
            .build()
            .context("build Monero RPC client")?;
        Ok(Self { client, base })
    }

    fn last_block_header(&self) -> Result<BlockHeaderJson> {
        let response: GetBlockHeaderResponse = self.rpc("get_last_block_header", json!({}))?;
        Ok(response.result.block_header)
    }

    fn block_header(&self, height: u64) -> Result<BlockHeaderJson> {
        let response: GetBlockHeaderResponse =
            self.rpc("get_block_header_by_height", json!({"height": height}))?;
        anyhow::ensure!(
            response.result.block_header.height == height,
            "Monero RPC substituted a different header height"
        );
        Ok(response.result.block_header)
    }

    fn fetch_transaction(&self, txid: B256) -> Result<GetTransactionEntry> {
        let response: GetTransactionsResponse = self.post(
            "gettransactions",
            json!({
                "txs_hashes": [hex::encode(txid)],
                "decode_as_json": false,
                "prune": false,
            }),
        )?;
        anyhow::ensure!(response.status == "OK", "Monero transaction RPC failed");
        anyhow::ensure!(
            response.missed_tx.is_empty(),
            "Monero transaction was not found"
        );
        anyhow::ensure!(
            response.txs.len() == 1,
            "Monero transaction RPC returned unexpected cardinality"
        );
        Ok(response.txs.into_iter().next().expect("length checked"))
    }

    fn fetch_block(&self, height: u64) -> Result<MoneroBlock> {
        let response: GetBlockResponse = self.rpc("get_block", json!({"height": height}))?;
        let blob = decode_hex_bounded("Monero block", &response.result.blob, 2 * 1024 * 1024)?;
        let block: monero::Block = deserialize(&blob).context("decode Monero block")?;
        let block_json: GetBlockJson =
            serde_json::from_str(&response.result.json).context("decode Monero block JSON")?;
        let rpc_hashes = block_json
            .tx_hashes
            .iter()
            .map(|hash| parse_b256(hash))
            .collect::<Result<Vec<_>>>()?;
        let decoded_hashes = block
            .tx_hashes
            .iter()
            .map(|hash| B256::from(hash.to_bytes()))
            .collect::<Vec<_>>();
        anyhow::ensure!(
            rpc_hashes == decoded_hashes,
            "Monero block JSON and decoded block disagree"
        );
        let header = self.block_header(height)?;
        let hash = B256::from(block.id().to_bytes());
        anyhow::ensure!(
            parse_b256(&header.hash)? == hash,
            "Monero parsed block hash and header RPC disagree"
        );
        Ok(MoneroBlock {
            timestamp: header.timestamp,
            block,
            hash,
        })
    }

    fn key_image_statuses(&self, key_images: &[B256]) -> Result<Vec<MoneroKeyImageStatusV1>> {
        let response: KeyImageSpentResponse = self.post(
            "is_key_image_spent",
            json!({
                "key_images": key_images.iter().map(hex::encode).collect::<Vec<_>>()
            }),
        )?;
        anyhow::ensure!(response.status == "OK", "Monero key-image RPC failed");
        anyhow::ensure!(
            response.spent_status.len() == key_images.len(),
            "Monero key-image status cardinality mismatch"
        );
        let mut statuses = key_images
            .iter()
            .zip(response.spent_status)
            .map(|(key_image, status)| {
                anyhow::ensure!(status <= 2, "unknown Monero key-image spent status");
                Ok(MoneroKeyImageStatusV1 {
                    key_image: *key_image,
                    spent: status != 0,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        statuses.sort_by_key(|status| status.key_image);
        Ok(statuses)
    }

    fn rpc<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        self.post(
            "json_rpc",
            json!({"jsonrpc": "2.0", "id": "postfiat", "method": method, "params": params}),
        )
    }

    fn post<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T> {
        let url = self.base.join(path).context("construct Monero RPC URL")?;
        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .context("send Monero RPC request")?
            .error_for_status()
            .context("Monero RPC HTTP error")?;
        let bytes = response.bytes().context("read Monero RPC response")?;
        anyhow::ensure!(
            bytes.len() <= MAX_RPC_RESPONSE_BYTES,
            "Monero RPC response exceeds bound"
        );
        serde_json::from_slice(&bytes).context("decode Monero RPC response")
    }
}

fn decode_hex_bounded(label: &str, value: &str, maximum: usize) -> Result<Vec<u8>> {
    let value = normalize_hex(value);
    anyhow::ensure!(
        value.len() % 2 == 0 && value.len() <= maximum.saturating_mul(2),
        "{label} hex length is out of bounds"
    );
    hex::decode(value).with_context(|| format!("decode {label} hex"))
}

fn parse_b256(value: &str) -> Result<B256> {
    let bytes = decode_hex_bounded("32-byte hash", value, 32)?;
    anyhow::ensure!(bytes.len() == 32, "hash is not 32 bytes");
    Ok(B256::from_slice(&bytes))
}

fn normalize_hex(value: &str) -> &str {
    value.trim().strip_prefix("0x").unwrap_or(value.trim())
}

fn monero_tree_branch(hashes: &[B256], mut index: usize) -> Result<Vec<B256>> {
    let count = hashes.len();
    anyhow::ensure!(
        count > 0 && index < count,
        "bad Monero transaction tree index"
    );
    if count == 1 {
        return Ok(Vec::new());
    }
    if count == 2 {
        return Ok(vec![hashes[index ^ 1]]);
    }
    let mut siblings = Vec::new();
    let mut width = tree_hash_count(count);
    let mut work = vec![B256::ZERO; width];
    let initial = 2 * width - count;
    work[..initial].copy_from_slice(&hashes[..initial]);
    let mut source = initial;
    let mut destination = initial;
    while destination < width {
        if index == source || index == source + 1 {
            siblings.push(hashes[if index == source { source + 1 } else { source }]);
            index = destination;
        }
        work[destination] = hash_pair(hashes[source], hashes[source + 1]);
        source += 2;
        destination += 1;
    }
    while width > 2 {
        width >>= 1;
        for position in 0..width {
            if index == 2 * position || index == 2 * position + 1 {
                siblings.push(
                    work[if index == 2 * position {
                        2 * position + 1
                    } else {
                        2 * position
                    }],
                );
                index = position;
            }
            work[position] = hash_pair(work[2 * position], work[2 * position + 1]);
        }
    }
    if index <= 1 {
        siblings.push(work[index ^ 1]);
    }
    Ok(siblings)
}

fn tree_hash_count(count: usize) -> usize {
    let mut power = 2usize;
    while power < count {
        power <<= 1;
    }
    power >> 1
}

fn hash_pair(left: B256, right: B256) -> B256 {
    let mut bytes = [0u8; 64];
    bytes[..32].copy_from_slice(left.as_slice());
    bytes[32..].copy_from_slice(right.as_slice());
    alloy_primitives::keccak256(bytes)
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
    if data.len() > MAX_RESERVE_PROOF_TEXT_BYTES {
        return;
    }
    let _ = parse_reserve_proof_v2_bytes(data);
    fuzz_monero_transaction_bytes(data);
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_reserve_proof_v2(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires POSTFIAT_MONERO_RESERVE_PROOF pointing to a wallet-created public proof"]
    fn external_wallet_reserve_proof_parses_with_public_code() {
        let path = std::env::var("POSTFIAT_MONERO_RESERVE_PROOF").unwrap();
        let value: ReserveProofJson =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let parsed = parse_reserve_proof_v2(&value.signature).unwrap();
        assert!(!parsed.entries.is_empty());
        assert!(!parsed.subaddr_spendkeys.is_empty());
    }

    #[test]
    #[ignore = "requires POSTFIAT_MONERO_DAEMON_URL for a public mainnet daemon"]
    fn public_daemon_reconstructs_finalized_block() {
        let rpc = MoneroRpc::new(&std::env::var("POSTFIAT_MONERO_DAEMON_URL").unwrap()).unwrap();
        let tip = rpc.last_block_header().unwrap();
        let finalized = rpc.fetch_block(tip.height - 12).unwrap();
        assert_ne!(finalized.hash, B256::ZERO);
        assert!(!finalized.transaction_hashes().is_empty());
    }

    #[test]
    fn parses_bounded_canonical_reserve_proof_shape() {
        let mut bytes = Vec::new();
        write_varint(1, &mut bytes);
        write_varint(0, &mut bytes);
        bytes.extend_from_slice(&[1; 32]);
        write_varint(7, &mut bytes);
        for value in 2u8..=7 {
            bytes.extend_from_slice(&[value; 32]);
        }
        write_varint(1, &mut bytes);
        write_varint(2, &mut bytes);
        for value in 8u8..=10 {
            bytes.extend_from_slice(&[value; 32]);
        }
        let signature = format!(
            "{RESERVE_PROOF_V2_PREFIX}{}",
            base58_monero::encode(&bytes).unwrap()
        );
        let parsed = parse_reserve_proof_v2(&signature).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].index_in_tx, 7);
        assert_eq!(parsed.subaddr_spendkeys.len(), 1);
    }

    #[test]
    fn parses_zero_output_reserve_proof_shape() {
        let mut bytes = Vec::new();
        write_varint(0, &mut bytes);
        write_varint(1, &mut bytes);
        write_varint(0, &mut bytes);
        bytes.extend_from_slice(&[8; 32]);
        bytes.extend_from_slice(&[9; 32]);
        bytes.extend_from_slice(&[10; 32]);
        let signature = format!(
            "{RESERVE_PROOF_V2_PREFIX}{}",
            base58_monero::encode(&bytes).unwrap()
        );
        let parsed = parse_reserve_proof_v2(&signature).unwrap();
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.subaddr_spendkeys.len(), 1);
    }

    #[test]
    fn rejects_noncanonical_varint_and_trailing_bytes() {
        let mut reader = ProofReader::new(&[0x80, 0x00]);
        assert!(reader.read_varint().is_err());
        let reader = ProofReader::new(&[0]);
        assert!(reader.finish().is_err());
    }

    #[test]
    fn tree_branches_match_public_verifier_vectors() {
        for count in 1usize..17 {
            let hashes = (0..count)
                .map(|index| B256::repeat_byte(u8::try_from(index + 1).unwrap()))
                .collect::<Vec<_>>();
            for index in 0..count {
                let branch = monero_tree_branch(&hashes, index).unwrap();
                assert!(branch.len() <= 5);
            }
        }
    }
}
