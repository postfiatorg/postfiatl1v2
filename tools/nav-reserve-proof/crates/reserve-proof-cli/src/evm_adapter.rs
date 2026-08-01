use std::{
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
    ed25519_evidence_signing_statement, ed25519_verifier_commitment,
    evm_checkpoint::{
        erc20_balance_slot, evm_owner_authorization_statement, evm_owner_commitment,
        EvmAccountProofV1, EvmErc20BalanceProofV1, EvmStateCheckpointCertificateV1,
        EvmStateCheckpointV1, EvmStorageProofV1, EVM_ERC20_ADAPTER_KIND_V1,
        MAX_EVM_PROOF_TOTAL_BYTES,
    },
    verify_observation_evidence, EvidenceDimensionV1, ReserveProofContextV1, SourceEvidenceV1,
    SourceManifestEntryV1, SourceManifestV1, SourceObservationV1, TrustClassV1, MAX_WITNESS_BYTES,
};
use serde::Deserialize;

const MAX_RPC_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Subcommand)]
pub enum AdapterCommand {
    /// Provider-neutral ERC-20 state-proof collection under a governed BFT
    /// checkpoint.
    EvmErc20 {
        #[command(subcommand)]
        command: EvmErc20Command,
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

pub fn run(command: AdapterCommand) -> Result<()> {
    match command {
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

fn load_source(
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

fn validate_rpc_url(raw: &str) -> Result<Url> {
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

fn rpc_call<T: serde::de::DeserializeOwned>(
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
        .with_context(|| format!("Ethereum RPC {method}"))?;
    anyhow::ensure!(
        response.status().is_success(),
        "Ethereum RPC {method} returned HTTP {}",
        response.status()
    );
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take((MAX_RPC_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= MAX_RPC_RESPONSE_BYTES,
        "Ethereum RPC {method} response exceeds {MAX_RPC_RESPONSE_BYTES} bytes"
    );
    let envelope: RpcEnvelope<T> =
        serde_json::from_slice(&bytes).with_context(|| format!("decode Ethereum RPC {method}"))?;
    anyhow::ensure!(
        envelope.jsonrpc == "2.0" && envelope.id == serde_json::json!(1),
        "Ethereum RPC {method} returned a mismatched JSON-RPC envelope"
    );
    if let Some(error) = envelope.error {
        bail!(
            "Ethereum RPC {method} failed with {}: {}",
            error.code,
            error.message
        );
    }
    envelope
        .result
        .with_context(|| format!("Ethereum RPC {method} omitted result"))
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

fn parse_address(label: &str, value: &str) -> Result<Address> {
    Address::from_str(value).with_context(|| format!("{label} is not a canonical EVM address"))
}

fn parse_b256(label: &str, value: &str) -> Result<B256> {
    B256::from_str(value).with_context(|| format!("{label} is not a 32-byte hex value"))
}

fn parse_u256(label: &str, value: &str) -> Result<U256> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    anyhow::ensure!(!digits.is_empty(), "{label} is empty");
    U256::from_str_radix(digits, 16).with_context(|| format!("{label} is not a hex quantity"))
}

fn parse_u64_quantity(label: &str, value: &str) -> Result<u64> {
    let parsed = parse_u256(label, value)?;
    u64::try_from(parsed).with_context(|| format!("{label} exceeds u64"))
}

fn decode_hex(label: &str, value: &str, expected_bytes: usize) -> Result<Vec<u8>> {
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

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file() && metadata.len() <= MAX_WITNESS_BYTES as u64,
        "input must be a regular file no larger than {MAX_WITNESS_BYTES} bytes: {}",
        path.display()
    );
    serde_json::from_slice(&fs::read(path)?).with_context(|| format!("decode {}", path.display()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
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
