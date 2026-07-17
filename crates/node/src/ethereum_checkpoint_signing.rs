use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use postfiat_bridge::{verify_ethereum_checkpoint_certificate, ROUTE_TRUST_CLASS_BFT_CHECKPOINT};
use postfiat_crypto_provider::{
    bytes_to_hex, hex_to_bytes, ml_dsa_65_sign_with_context, ml_dsa_65_verify_with_context,
    ML_DSA_65_ALGORITHM,
};
use postfiat_storage::{atomic_write, NodeStore};
use postfiat_types::{
    EthereumCheckpointCertificateV1, EthereumCheckpointVoteV1, EthereumFinalizedCheckpointV1,
    FastSwapCommitteeV1, FastSwapOpaqueHashV1, ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha3::{Digest, Keccak256};
use zeroize::Zeroizing;

use crate::{read_json_file, read_validator_key_file, validator_key_record};

const ETHEREUM_CHECKPOINT_SIGNING_STATE_SCHEMA_V1: &str =
    "postfiat-ethereum-checkpoint-signing-state-v1";
const ETHEREUM_CHECKPOINT_SIGNING_STATE_DIR: &str = "ethereum-checkpoint-signing";
static SIGNING_STATE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EthereumCheckpointSigningStateV1 {
    schema: String,
    validator_id: String,
    checkpoint: EthereumFinalizedCheckpointV1,
    vote: Option<EthereumCheckpointVoteV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumCheckpointVoteSignOptions {
    pub data_dir: PathBuf,
    pub checkpoint_file: PathBuf,
    pub ethereum_rpc: String,
    pub validator: String,
    pub validator_key_file: PathBuf,
    pub vote_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumCheckpointCertificateAssembleOptions {
    pub data_dir: PathBuf,
    pub checkpoint_file: PathBuf,
    pub vote_files: Vec<PathBuf>,
    pub certificate_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthereumCheckpointObserveOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub ethereum_rpc: String,
    pub block_number: Option<u64>,
    pub checkpoint_file: PathBuf,
}

pub fn observe_ethereum_checkpoint(
    options: EthereumCheckpointObserveOptions,
) -> io::Result<EthereumFinalizedCheckpointV1> {
    let store = NodeStore::new(&options.data_dir);
    let ledger = store.read_ledger()?;
    let route = ledger
        .pftl_uniswap_route(&options.route_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "checkpoint route is not live"))?;
    route.validate().map_err(invalid_data)?;
    if route.route_trust_class != ROUTE_TRUST_CLASS_BFT_CHECKPOINT {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "checkpoint route does not use the BFT_CHECKPOINT trust class",
        ));
    }
    let policy = route.ethereum_verification_policy.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "checkpoint route has no governed Ethereum verification policy",
        )
    })?;
    policy.validate().map_err(invalid_data)?;
    let committee = ledger
        .fastswap_committees
        .iter()
        .find(|committee| {
            committee.domain.committee_epoch == policy.authority_epoch
                && committee.domain.committee_root == policy.committee_root
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "checkpoint route committee is not present in live governed state",
            )
        })?;
    committee.validate().map_err(invalid_data)?;

    let endpoint = EthereumRpcEndpoint::parse(&options.ethereum_rpc)?;
    let chain_id = rpc_hex_u64(&endpoint, "eth_chainId", serde_json::json!([]))?;
    if chain_id != route.ethereum_chain_id {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Ethereum RPC chain ID does not match the governed route",
        ));
    }
    let observed_head_number = rpc_hex_u64(&endpoint, "eth_blockNumber", serde_json::json!([]))?;
    let latest_finalized_block = observed_head_number
        .checked_sub(u64::from(policy.minimum_confirmations))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum head is below the governed confirmation depth",
            )
        })?;
    let block_number = options.block_number.unwrap_or(latest_finalized_block);
    if block_number == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governed confirmation depth resolves to Ethereum block zero",
        ));
    }
    if block_number > latest_finalized_block {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "requested Ethereum checkpoint block has not reached the governed confirmation depth",
        ));
    }
    let block_tag = format!("0x{block_number:x}");
    let block = ethereum_rpc_call(
        &endpoint,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    let returned_number = value_hex_u64(&block, "number")?;
    if returned_number != block_number {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum RPC returned a block at the wrong height",
        ));
    }
    let block_hash = value_hex_array::<32>(&block, "hash")?;
    let receipts_root = value_hex_array::<32>(&block, "receiptsRoot")?;
    let handoff_controller = exact_evm_address("handoff controller", &route.handoff_controller)?;
    let wrapped_navcoin = exact_evm_address("wrapped NAVCoin", &route.wrapped_navcoin_token)?;
    let controller_code = ethereum_rpc_hex_bytes(
        &endpoint,
        "eth_getCode",
        serde_json::json!([route.handoff_controller, format!("0x{block_number:x}")]),
    )?;
    let wrapped_code = ethereum_rpc_hex_bytes(
        &endpoint,
        "eth_getCode",
        serde_json::json!([route.wrapped_navcoin_token, format!("0x{block_number:x}")]),
    )?;
    if controller_code.is_empty() || wrapped_code.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governed Ethereum contract has empty runtime code at the checkpoint",
        ));
    }
    let controller_code_hash: [u8; 32] = Keccak256::digest(&controller_code).into();
    let wrapped_code_hash: [u8; 32] = Keccak256::digest(&wrapped_code).into();
    if controller_code_hash != policy.handoff_controller_code_hash
        || wrapped_code_hash != policy.wrapped_navcoin_code_hash
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Ethereum runtime code hash does not match the governed route policy",
        ));
    }

    let checkpoint = EthereumFinalizedCheckpointV1 {
        schema_version: postfiat_types::ETHEREUM_CHECKPOINT_SCHEMA_V1,
        pftl_domain: committee.domain.chain.clone(),
        route_id: route.route_id.clone(),
        route_config_digest: FastSwapOpaqueHashV1(exact_hex(
            "route config digest",
            &route.route_config_digest,
        )?),
        ethereum_chain_id: chain_id,
        block_number,
        block_hash,
        receipts_root,
        observed_head_number,
        minimum_confirmations: policy.minimum_confirmations,
        authority_epoch: policy.authority_epoch,
        committee_root: policy.committee_root,
        handoff_controller,
        wrapped_navcoin_token: wrapped_navcoin,
        handoff_controller_code_hash: controller_code_hash,
        wrapped_navcoin_code_hash: wrapped_code_hash,
    };
    governed_committee_for_checkpoint(&options.data_dir, &checkpoint)?;
    write_json(&options.checkpoint_file, &checkpoint)?;
    Ok(checkpoint)
}

pub fn sign_ethereum_checkpoint_vote(
    options: EthereumCheckpointVoteSignOptions,
) -> io::Result<EthereumCheckpointVoteV1> {
    let checkpoint: EthereumFinalizedCheckpointV1 =
        read_json_file(&options.checkpoint_file, "Ethereum checkpoint")?;
    let committee = governed_committee_for_checkpoint(&options.data_dir, &checkpoint)?;
    verify_checkpoint_against_ethereum_rpc(&checkpoint, &options.ethereum_rpc)?;
    let validator = committee
        .validators
        .iter()
        .find(|validator| validator.validator_id == options.validator)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "checkpoint signer is not in the governed route committee",
            )
        })?;

    let key_file = read_validator_key_file(&options.validator_key_file)?;
    let key_record = validator_key_record(&key_file, &options.validator)?;
    if key_record.algorithm_id != ML_DSA_65_ALGORITHM
        || hex_to_bytes(&key_record.public_key_hex).map_err(invalid_data)? != validator.public_key
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "checkpoint signing key does not match the governed route committee",
        ));
    }

    let signing_state_path =
        ethereum_checkpoint_signing_state_path(&options.data_dir, &checkpoint, &options.validator);
    let mut signing_state = load_or_create_checkpoint_signing_state(
        &signing_state_path,
        EthereumCheckpointSigningStateV1 {
            schema: ETHEREUM_CHECKPOINT_SIGNING_STATE_SCHEMA_V1.to_string(),
            validator_id: options.validator.clone(),
            checkpoint: checkpoint.clone(),
            vote: None,
        },
    )?;
    validate_checkpoint_signing_state(
        &signing_state,
        &checkpoint,
        &options.validator,
        &validator.public_key,
    )?;
    if let Some(vote) = signing_state.vote {
        write_json(&options.vote_file, &vote)?;
        return Ok(vote);
    }

    let mut vote = EthereumCheckpointVoteV1 {
        validator_id: options.validator,
        signature: Vec::new(),
    };
    let signing_bytes = vote.signing_bytes(&checkpoint).map_err(invalid_data)?;
    let private_key =
        Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
    vote.signature = ml_dsa_65_sign_with_context(
        &private_key,
        &signing_bytes,
        ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
    )
    .map_err(invalid_data)?;

    signing_state.vote = Some(vote.clone());
    write_private_json(&signing_state_path, &signing_state)?;
    write_json(&options.vote_file, &vote)?;
    Ok(vote)
}

fn verify_checkpoint_against_ethereum_rpc(
    checkpoint: &EthereumFinalizedCheckpointV1,
    ethereum_rpc: &str,
) -> io::Result<()> {
    let endpoint = EthereumRpcEndpoint::parse(ethereum_rpc)?;
    let chain_id = rpc_hex_u64(&endpoint, "eth_chainId", serde_json::json!([]))?;
    if chain_id != checkpoint.ethereum_chain_id {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Ethereum RPC chain ID does not match the checkpoint",
        ));
    }
    let current_head = rpc_hex_u64(&endpoint, "eth_blockNumber", serde_json::json!([]))?;
    if current_head < checkpoint.observed_head_number {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum RPC head is behind the checkpoint observation",
        ));
    }
    let block_tag = format!("0x{:x}", checkpoint.block_number);
    let block = ethereum_rpc_call(
        &endpoint,
        "eth_getBlockByNumber",
        serde_json::json!([block_tag, false]),
    )?;
    if value_hex_u64(&block, "number")? != checkpoint.block_number
        || value_hex_array::<32>(&block, "hash")? != checkpoint.block_hash
        || value_hex_array::<32>(&block, "receiptsRoot")? != checkpoint.receipts_root
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Ethereum RPC block hash or receipts root does not match the checkpoint",
        ));
    }
    let controller = format!("0x{}", bytes_to_hex(&checkpoint.handoff_controller));
    let wrapped = format!("0x{}", bytes_to_hex(&checkpoint.wrapped_navcoin_token));
    let controller_code = ethereum_rpc_hex_bytes(
        &endpoint,
        "eth_getCode",
        serde_json::json!([controller, format!("0x{:x}", checkpoint.block_number)]),
    )?;
    let wrapped_code = ethereum_rpc_hex_bytes(
        &endpoint,
        "eth_getCode",
        serde_json::json!([wrapped, format!("0x{:x}", checkpoint.block_number)]),
    )?;
    let controller_hash: [u8; 32] = Keccak256::digest(&controller_code).into();
    let wrapped_hash: [u8; 32] = Keccak256::digest(&wrapped_code).into();
    if controller_code.is_empty()
        || wrapped_code.is_empty()
        || controller_hash != checkpoint.handoff_controller_code_hash
        || wrapped_hash != checkpoint.wrapped_navcoin_code_hash
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Ethereum RPC runtime code does not match the checkpoint",
        ));
    }
    Ok(())
}

pub fn assemble_ethereum_checkpoint_certificate(
    options: EthereumCheckpointCertificateAssembleOptions,
) -> io::Result<EthereumCheckpointCertificateV1> {
    let checkpoint: EthereumFinalizedCheckpointV1 =
        read_json_file(&options.checkpoint_file, "Ethereum checkpoint")?;
    let committee = governed_committee_for_checkpoint(&options.data_dir, &checkpoint)?;
    if options.vote_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one Ethereum checkpoint vote file is required",
        ));
    }
    let mut votes = options
        .vote_files
        .iter()
        .map(|path| read_json_file(path, "Ethereum checkpoint vote"))
        .collect::<io::Result<Vec<EthereumCheckpointVoteV1>>>()?;
    votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    let certificate = EthereumCheckpointCertificateV1 { checkpoint, votes };
    verify_ethereum_checkpoint_certificate(&committee, &certificate).map_err(invalid_data)?;
    write_json(&options.certificate_file, &certificate)?;
    Ok(certificate)
}

fn ethereum_checkpoint_signing_state_path(
    data_dir: &Path,
    checkpoint: &EthereumFinalizedCheckpointV1,
    validator_id: &str,
) -> PathBuf {
    let mut hasher = Keccak256::new();
    hasher.update(b"postfiat.ethereum-checkpoint.signing-state.v1");
    hasher.update(checkpoint.pftl_domain.genesis_hash.0);
    hasher.update((checkpoint.route_id.len() as u64).to_be_bytes());
    hasher.update(checkpoint.route_id.as_bytes());
    hasher.update(checkpoint.authority_epoch.to_be_bytes());
    hasher.update(checkpoint.block_number.to_be_bytes());
    hasher.update((validator_id.len() as u64).to_be_bytes());
    hasher.update(validator_id.as_bytes());
    data_dir
        .join(ETHEREUM_CHECKPOINT_SIGNING_STATE_DIR)
        .join(format!("{}.json", bytes_to_hex(&hasher.finalize())))
}

fn load_or_create_checkpoint_signing_state(
    path: &Path,
    intended: EthereumCheckpointSigningStateV1,
) -> io::Result<EthereumCheckpointSigningStateV1> {
    if path.exists() {
        return read_json_file(path, "Ethereum checkpoint signing state");
    }
    let mut contents = serde_json::to_vec_pretty(&intended).map_err(invalid_data)?;
    contents.push(b'\n');
    match create_file_once(path, &contents) {
        Ok(()) => Ok(intended),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            read_json_file(path, "Ethereum checkpoint signing state")
        }
        Err(error) => Err(error),
    }
}

fn create_file_once(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "checkpoint signing state path has no parent",
        )
    })?;
    fs::create_dir_all(parent)?;
    let counter = SIGNING_STATE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(
        ".checkpoint-signing-{}-{counter}.tmp",
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temp)?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    drop(file);
    match fs::hard_link(&temp, path) {
        Ok(()) => {
            fs::remove_file(&temp)?;
            crate::set_private_file_permissions(path)?;
            sync_directory(parent)
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    }
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn validate_checkpoint_signing_state(
    state: &EthereumCheckpointSigningStateV1,
    checkpoint: &EthereumFinalizedCheckpointV1,
    validator_id: &str,
    validator_public_key: &[u8],
) -> io::Result<()> {
    if state.schema != ETHEREUM_CHECKPOINT_SIGNING_STATE_SCHEMA_V1
        || state.validator_id != validator_id
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum checkpoint signing state has the wrong domain",
        ));
    }
    if state.checkpoint != *checkpoint {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "validator already recorded a conflicting Ethereum checkpoint at this route, epoch, and height",
        ));
    }
    if let Some(vote) = &state.vote {
        if vote.validator_id != validator_id
            || !ml_dsa_65_verify_with_context(
                validator_public_key,
                &vote.signing_bytes(checkpoint).map_err(invalid_data)?,
                &vote.signature,
                ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
            )
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "persisted Ethereum checkpoint vote does not verify",
            ));
        }
    }
    Ok(())
}

fn write_private_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    write_json(path, value)?;
    crate::set_private_file_permissions(path)
}

fn governed_committee_for_checkpoint(
    data_dir: &std::path::Path,
    checkpoint: &EthereumFinalizedCheckpointV1,
) -> io::Result<FastSwapCommitteeV1> {
    checkpoint.validate().map_err(invalid_data)?;
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let route = ledger
        .pftl_uniswap_route(&checkpoint.route_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "checkpoint route is not live"))?;
    route.validate().map_err(invalid_data)?;
    if route.route_trust_class != ROUTE_TRUST_CLASS_BFT_CHECKPOINT {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "checkpoint route does not use the BFT_CHECKPOINT trust class",
        ));
    }
    let policy = route.ethereum_verification_policy.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "checkpoint route has no governed Ethereum verification policy",
        )
    })?;
    policy.validate().map_err(invalid_data)?;
    let committee = ledger
        .fastswap_committees
        .iter()
        .find(|committee| {
            committee.domain.committee_epoch == policy.authority_epoch
                && committee.domain.committee_root == policy.committee_root
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "checkpoint route committee is not present in live governed state",
            )
        })?;
    committee.validate().map_err(invalid_data)?;

    let genesis_hash =
        exact_hex::<48>("genesis hash", &postfiat_execution::genesis_hash(&genesis))?;
    let route_digest = exact_hex::<48>("route config digest", &route.route_config_digest)?;
    let handoff_controller = exact_evm_address("handoff controller", &route.handoff_controller)?;
    let wrapped_navcoin = exact_evm_address("wrapped NAVCoin", &route.wrapped_navcoin_token)?;
    let expected_domain = &committee.domain.chain;
    let route_matches = checkpoint.pftl_domain.chain_id == genesis.chain_id
        && checkpoint.pftl_domain.genesis_hash == FastSwapOpaqueHashV1(genesis_hash)
        && checkpoint.pftl_domain.protocol_version == genesis.protocol_version
        && &checkpoint.pftl_domain == expected_domain
        && checkpoint.route_config_digest == FastSwapOpaqueHashV1(route_digest)
        && checkpoint.ethereum_chain_id == route.ethereum_chain_id
        && checkpoint.minimum_confirmations == policy.minimum_confirmations
        && checkpoint.authority_epoch == policy.authority_epoch
        && checkpoint.committee_root == policy.committee_root
        && checkpoint.handoff_controller == handoff_controller
        && checkpoint.wrapped_navcoin_token == wrapped_navcoin
        && checkpoint.handoff_controller_code_hash == policy.handoff_controller_code_hash
        && checkpoint.wrapped_navcoin_code_hash == policy.wrapped_navcoin_code_hash;
    if !route_matches {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Ethereum checkpoint does not exactly match the live governed route",
        ));
    }
    Ok(committee.clone())
}

const ETHEREUM_RPC_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const ETHEREUM_RPC_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EthereumRpcEndpoint {
    host: String,
    port: u16,
    path: String,
}

impl EthereumRpcEndpoint {
    pub(crate) fn parse(value: &str) -> io::Result<Self> {
        if value
            .as_bytes()
            .iter()
            .any(|byte| *byte <= b' ' || *byte == 0x7f)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Ethereum RPC endpoint contains whitespace or control bytes",
            ));
        }
        let rest = value.strip_prefix("http://").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Ethereum checkpoint observation requires an http:// endpoint for a local execution client",
            )
        })?;
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, "/".to_string()), |(authority, path)| {
                (authority, format!("/{path}"))
            });
        if authority.is_empty() || authority.contains('@') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Ethereum RPC endpoint authority is invalid",
            ));
        }
        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let (host, suffix) = bracketed.split_once(']').ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid bracketed RPC host")
            })?;
            let port = suffix.strip_prefix(':').ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Ethereum RPC port is required")
            })?;
            (host.to_string(), parse_rpc_port(port)?)
        } else {
            let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Ethereum RPC port is required")
            })?;
            if host.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Ethereum RPC host is empty",
                ));
            }
            (host.to_string(), parse_rpc_port(port)?)
        };
        Ok(Self { host, port, path })
    }
}

fn parse_rpc_port(value: &str) -> io::Result<u16> {
    value
        .parse::<u16>()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Ethereum RPC port must be a nonzero u16",
            )
        })
        .and_then(|port| {
            if port == 0 {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Ethereum RPC port must be a nonzero u16",
                ))
            } else {
                Ok(port)
            }
        })
}

pub(crate) fn ethereum_rpc_call(
    endpoint: &EthereumRpcEndpoint,
    method: &str,
    params: Value,
) -> io::Result<Value> {
    ethereum_rpc_call_with_limit(endpoint, method, params, ETHEREUM_RPC_MAX_RESPONSE_BYTES)
}

pub(crate) fn ethereum_rpc_call_with_limit(
    endpoint: &EthereumRpcEndpoint,
    method: &str,
    params: Value,
    max_response_bytes: usize,
) -> io::Result<Value> {
    if max_response_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Ethereum RPC response limit must be nonzero",
        ));
    }
    let request_body = serde_json::to_vec(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    }))
    .map_err(invalid_data)?;
    let addresses = (endpoint.host.as_str(), endpoint.port)
        .to_socket_addrs()?
        .collect::<Vec<_>>();
    let mut last_error = None;
    let mut stream = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, ETHEREUM_RPC_TIMEOUT) {
            Ok(connected) => {
                stream = Some(connected);
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let mut stream = stream.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Ethereum RPC host has no addresses",
            )
        })
    })?;
    stream.set_read_timeout(Some(ETHEREUM_RPC_TIMEOUT))?;
    stream.set_write_timeout(Some(ETHEREUM_RPC_TIMEOUT))?;
    write!(
        stream,
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        endpoint.path,
        endpoint.host,
        endpoint.port,
        request_body.len()
    )?;
    stream.write_all(&request_body)?;
    stream.flush()?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        if response.len().saturating_add(count) > max_response_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum RPC response exceeds the bounded response limit",
            ));
        }
        response.extend_from_slice(&chunk[..count]);
    }
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed HTTP response"))?;
    let headers = std::str::from_utf8(&response[..header_end]).map_err(invalid_data)?;
    let status = headers.lines().next().unwrap_or_default();
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Ethereum RPC returned HTTP status `{status}`"),
        ));
    }
    if headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "chunked Ethereum RPC responses are not accepted by the bounded observer",
        ));
    }
    let body = &response[header_end..];
    if let Some(expected_length) = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    }) {
        if body.len() != expected_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Ethereum RPC Content-Length does not match the response body",
            ));
        }
    }
    let envelope: Value = serde_json::from_slice(body).map_err(invalid_data)?;
    if envelope.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || envelope.get("id").and_then(Value::as_u64) != Some(1)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum RPC response has the wrong JSON-RPC domain",
        ));
    }
    if let Some(error) = envelope.get("error") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Ethereum RPC {method} failed: {error}"),
        ));
    }
    envelope.get("result").cloned().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Ethereum RPC {method} response has no result"),
        )
    })
}

fn rpc_hex_u64(endpoint: &EthereumRpcEndpoint, method: &str, params: Value) -> io::Result<u64> {
    let result = ethereum_rpc_call(endpoint, method, params)?;
    parse_hex_u64(result.as_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum RPC result is not a hex string",
        )
    })?)
}

fn ethereum_rpc_hex_bytes(
    endpoint: &EthereumRpcEndpoint,
    method: &str,
    params: Value,
) -> io::Result<Vec<u8>> {
    let result = ethereum_rpc_call(endpoint, method, params)?;
    let encoded = result.as_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum RPC result is not hex bytes",
        )
    })?;
    hex_to_bytes(encoded.strip_prefix("0x").unwrap_or(encoded)).map_err(invalid_data)
}

fn value_hex_u64(value: &Value, field: &'static str) -> io::Result<u64> {
    parse_hex_u64(value.get(field).and_then(Value::as_str).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Ethereum block has no `{field}` hex field"),
        )
    })?)
}

fn value_hex_array<const N: usize>(value: &Value, field: &'static str) -> io::Result<[u8; N]> {
    exact_hex(
        field,
        value.get(field).and_then(Value::as_str).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Ethereum block has no `{field}` hex field"),
            )
        })?,
    )
}

fn parse_hex_u64(value: &str) -> io::Result<u64> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum quantity is missing 0x",
        )
    })?;
    if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Ethereum quantity is not minimally encoded",
        ));
    }
    u64::from_str_radix(digits, 16).map_err(invalid_data)
}

fn exact_evm_address(field: &'static str, value: &str) -> io::Result<[u8; 20]> {
    exact_hex(field, value.strip_prefix("0x").unwrap_or(value))
}

fn exact_hex<const N: usize>(field: &'static str, value: &str) -> io::Result<[u8; N]> {
    hex_to_bytes(value.strip_prefix("0x").unwrap_or(value))
        .map_err(invalid_data)?
        .try_into()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{field} must contain exactly {N} bytes"),
            )
        })
}

fn write_json(path: &std::path::Path, value: &impl serde::Serialize) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn invalid_data(error: impl std::fmt::Debug) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::TcpListener;
    use std::sync::{Arc, Barrier};

    use postfiat_crypto_provider::{
        address_from_public_key, bytes_to_hex, hex_to_bytes, ml_dsa_65_keygen_from_seed,
        ml_dsa_65_sign, MlDsa65KeyPair,
    };
    use postfiat_types::{
        pftl_uniswap_non_consumption_proof_hash, pftl_uniswap_return_burn_id_from_fields, Account,
        AssetDefinition, AssetTransactionOperation, EthereumExternalEventProofV1,
        EthereumReceiptProofV1, EthereumRouteVerificationPolicyV1, FastSwapChainDomainV1,
        FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1, FastSwapValidatorV1, Genesis,
        LedgerState, NavTrackedAsset, PftlUniswapConsensusRouteState,
        PftlUniswapDestinationConsumeOperation, PftlUniswapExportDebitOperation,
        PftlUniswapPrimarySubscribeOperation, PftlUniswapRefundSourceOperation,
        PftlUniswapReturnImportOperation, SignedAssetTransaction, TrustLine,
        UnsignedAssetTransaction, ADDRESS_NAMESPACE, FASTSWAP_SCHEMA_VERSION_V1,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND, PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND, PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT,
    };

    use super::*;
    use crate::{
        build_ethereum_receipt_proof, EthereumReceiptProofBuildOptions, ValidatorKeyFile,
        ValidatorKeyRecord,
    };

    #[test]
    fn isolated_checkpoint_votes_require_live_route_and_assemble_exact_quorum() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-ethereum-checkpoint-signing-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create checkpoint test dir");

        let genesis = Genesis::new("postfiat-ethereum-checkpoint-signing-test");
        let keys = (0_u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: genesis.chain_id.clone(),
                    genesis_hash: FastSwapOpaqueHashV1(
                        exact_hex("genesis hash", &postfiat_execution::genesis_hash(&genesis))
                            .expect("genesis hash bytes"),
                    ),
                    protocol_version: genesis.protocol_version,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 9,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 4,
                quorum: 3,
            },
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        let controller_code = vec![0x60, 0x01, 0x60, 0x00];
        let wrapped_code = vec![0x60, 0x02, 0x60, 0x00];
        let policy = EthereumRouteVerificationPolicyV1 {
            authority_epoch: committee.domain.committee_epoch,
            committee_root: committee.domain.committee_root,
            minimum_confirmations: 12,
            handoff_controller_code_hash: Keccak256::digest(&controller_code).into(),
            wrapped_navcoin_code_hash: Keccak256::digest(&wrapped_code).into(),
        };
        let route = PftlUniswapConsensusRouteState {
            route_id: "pftl-uniswap-a651".to_string(),
            route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
            route_config_digest: "14".repeat(48),
            route_trust_class: ROUTE_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
            native_nav_asset_id: "11".repeat(48),
            settlement_asset_id: "22".repeat(48),
            handoff_controller: format!("0x{}", "33".repeat(20)),
            settlement_adapter: format!("0x{}", "34".repeat(20)),
            wrapped_navcoin_token: format!("0x{}", "35".repeat(20)),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000,
            packet_notional_cap_atoms: 100,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 12,
            ethereum_verification_policy: Some(policy.clone()),
            authorized_valid_supply_atoms: 0,
            pftl_spendable_supply_atoms: 0,
            native_spendable_balances_atoms: BTreeMap::new(),
            ethereum_spendable_supply_atoms: 0,
            other_registered_venue_supply_atoms: 0,
            outstanding_bridge_claims_atoms: 0,
            pending_return_import_claims_atoms: 0,
            settlement_reserve_atoms: 0,
            primary_subscription_nonces: BTreeMap::new(),
            export_packets: BTreeMap::new(),
            export_nonces: BTreeMap::new(),
            return_imports: BTreeMap::new(),
            paused: false,
        };
        let mut ledger = LedgerState::empty();
        ledger.pftl_uniswap_routes.push(route);
        ledger.fastswap_committees.push(committee.clone());
        let store = NodeStore::new(&root);
        store.write_genesis(&genesis).expect("write genesis");
        store.write_ledger(&ledger).expect("write ledger");

        let (rpc, rpc_thread) = spawn_test_ethereum_rpc(
            controller_code.clone(),
            wrapped_code.clone(),
            [0x51; 32],
            [0x52; 32],
            100,
            120,
            5,
        );
        let checkpoint_file = root.join("checkpoint.json");
        let checkpoint = observe_ethereum_checkpoint(EthereumCheckpointObserveOptions {
            data_dir: root.clone(),
            route_id: "pftl-uniswap-a651".to_string(),
            ethereum_rpc: rpc,
            block_number: Some(100),
            checkpoint_file: checkpoint_file.clone(),
        })
        .expect("observe governed Ethereum checkpoint");
        rpc_thread.join().expect("Ethereum RPC test server");
        assert_eq!(checkpoint.block_number, 100);
        assert_eq!(checkpoint.observed_head_number, 120);
        assert_eq!(checkpoint.block_hash, [0x51; 32]);
        assert_eq!(checkpoint.receipts_root, [0x52; 32]);

        let (unfinalized_rpc, unfinalized_thread) = spawn_test_ethereum_rpc(
            controller_code.clone(),
            wrapped_code.clone(),
            [0x53; 32],
            [0x54; 32],
            119,
            120,
            2,
        );
        let unfinalized_file = root.join("unfinalized-checkpoint.json");
        let unfinalized = observe_ethereum_checkpoint(EthereumCheckpointObserveOptions {
            data_dir: root.clone(),
            route_id: "pftl-uniswap-a651".to_string(),
            ethereum_rpc: unfinalized_rpc,
            block_number: Some(119),
            checkpoint_file: unfinalized_file.clone(),
        });
        unfinalized_thread
            .join()
            .expect("unfinalized Ethereum RPC test server");
        assert!(unfinalized.is_err());
        assert!(!unfinalized_file.exists());

        let (wrong_code_rpc, wrong_code_thread) = spawn_test_ethereum_rpc(
            vec![0x60, 0xff],
            wrapped_code.clone(),
            [0x51; 32],
            [0x52; 32],
            100,
            120,
            5,
        );
        let wrong_code_file = root.join("wrong-code-checkpoint.json");
        let wrong_code = observe_ethereum_checkpoint(EthereumCheckpointObserveOptions {
            data_dir: root.clone(),
            route_id: "pftl-uniswap-a651".to_string(),
            ethereum_rpc: wrong_code_rpc,
            block_number: Some(100),
            checkpoint_file: wrong_code_file.clone(),
        });
        wrong_code_thread
            .join()
            .expect("wrong-code Ethereum RPC test server");
        assert!(wrong_code.is_err());
        assert!(!wrong_code_file.exists());

        let mut vote_files = Vec::new();
        for (index, key) in keys.iter().enumerate().take(3) {
            let validator = format!("validator-{index}");
            let key_file = root.join(format!("{validator}.key.json"));
            write_json(
                &key_file,
                &ValidatorKeyFile {
                    validators: vec![ValidatorKeyRecord {
                        node_id: validator.clone(),
                        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                        public_key_hex: bytes_to_hex(&key.public_key),
                        private_key_hex: bytes_to_hex(&key.private_key),
                    }],
                },
            )
            .expect("write isolated key");
            crate::set_private_file_permissions(&key_file).expect("protect isolated key");
            let vote_file = root.join(format!("{validator}.vote.json"));
            sign_checkpoint_vote_with_test_rpc(
                EthereumCheckpointVoteSignOptions {
                    data_dir: root.clone(),
                    checkpoint_file: checkpoint_file.clone(),
                    ethereum_rpc: String::new(),
                    validator,
                    validator_key_file: key_file,
                    vote_file: vote_file.clone(),
                },
                controller_code.clone(),
                wrapped_code.clone(),
                checkpoint.block_hash,
                checkpoint.receipts_root,
                checkpoint.block_number,
                checkpoint.observed_head_number,
            )
            .expect("sign checkpoint vote");
            vote_files.push(vote_file);
        }

        let replay_vote_file = root.join("validator-0-replay.vote.json");
        let replay_vote = sign_checkpoint_vote_with_test_rpc(
            EthereumCheckpointVoteSignOptions {
                data_dir: root.clone(),
                checkpoint_file: checkpoint_file.clone(),
                ethereum_rpc: String::new(),
                validator: "validator-0".to_string(),
                validator_key_file: root.join("validator-0.key.json"),
                vote_file: replay_vote_file,
            },
            controller_code.clone(),
            wrapped_code.clone(),
            checkpoint.block_hash,
            checkpoint.receipts_root,
            checkpoint.block_number,
            checkpoint.observed_head_number,
        )
        .expect("exact checkpoint replay is idempotent");
        let first_vote: EthereumCheckpointVoteV1 =
            read_json_file(&vote_files[0], "first checkpoint vote").expect("read first vote");
        assert_eq!(replay_vote, first_vote);

        let mut conflicting_checkpoint = checkpoint.clone();
        conflicting_checkpoint.block_hash[0] ^= 1;
        conflicting_checkpoint.receipts_root[0] ^= 1;
        let conflicting_checkpoint_file = root.join("conflicting-checkpoint.json");
        write_json(&conflicting_checkpoint_file, &conflicting_checkpoint)
            .expect("write conflicting checkpoint");
        let conflicting_vote_file = root.join("conflicting.vote.json");
        let conflicting_vote = sign_checkpoint_vote_with_test_rpc(
            EthereumCheckpointVoteSignOptions {
                data_dir: root.clone(),
                checkpoint_file: conflicting_checkpoint_file,
                ethereum_rpc: String::new(),
                validator: "validator-0".to_string(),
                validator_key_file: root.join("validator-0.key.json"),
                vote_file: conflicting_vote_file.clone(),
            },
            controller_code.clone(),
            wrapped_code.clone(),
            conflicting_checkpoint.block_hash,
            conflicting_checkpoint.receipts_root,
            conflicting_checkpoint.block_number,
            conflicting_checkpoint.observed_head_number,
        );
        assert!(conflicting_vote.is_err());
        assert!(!conflicting_vote_file.exists());

        let validator_3_key_file = root.join("validator-3.key.json");
        write_json(
            &validator_3_key_file,
            &ValidatorKeyFile {
                validators: vec![ValidatorKeyRecord {
                    node_id: "validator-3".to_string(),
                    algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                    public_key_hex: bytes_to_hex(&keys[3].public_key),
                    private_key_hex: bytes_to_hex(&keys[3].private_key),
                }],
            },
        )
        .expect("write validator-3 isolated key");
        crate::set_private_file_permissions(&validator_3_key_file)
            .expect("protect validator-3 isolated key");

        let unobserved_vote_file = root.join("validator-3-unobserved.vote.json");
        let (unobserved_rpc, unobserved_rpc_thread) = spawn_test_ethereum_rpc(
            controller_code.clone(),
            wrapped_code.clone(),
            checkpoint.block_hash,
            checkpoint.receipts_root,
            checkpoint.block_number,
            checkpoint.observed_head_number,
            3,
        );
        let unobserved_vote = sign_ethereum_checkpoint_vote(EthereumCheckpointVoteSignOptions {
            data_dir: root.clone(),
            checkpoint_file: root.join("conflicting-checkpoint.json"),
            ethereum_rpc: unobserved_rpc,
            validator: "validator-3".to_string(),
            validator_key_file: validator_3_key_file.clone(),
            vote_file: unobserved_vote_file.clone(),
        });
        unobserved_rpc_thread
            .join()
            .expect("unobserved-candidate Ethereum RPC test server");
        assert!(unobserved_vote.is_err());
        assert!(!unobserved_vote_file.exists());
        assert!(
            !ethereum_checkpoint_signing_state_path(&root, &conflicting_checkpoint, "validator-3")
                .exists(),
            "an RPC-mismatched candidate must not create durable signing intent"
        );

        let barrier = Arc::new(Barrier::new(2));
        let contenders = [
            (
                checkpoint_file.clone(),
                root.join("validator-3-a.vote.json"),
            ),
            (
                root.join("conflicting-checkpoint.json"),
                root.join("validator-3-b.vote.json"),
            ),
        ]
        .into_iter()
        .map(|(candidate, output)| {
            let root = root.clone();
            let key_file = validator_3_key_file.clone();
            let barrier = Arc::clone(&barrier);
            let controller_code = controller_code.clone();
            let wrapped_code = wrapped_code.clone();
            let candidate_checkpoint: EthereumFinalizedCheckpointV1 =
                read_json_file(&candidate, "checkpoint signing contender")
                    .expect("read checkpoint signing contender");
            std::thread::spawn(move || {
                barrier.wait();
                let result = sign_checkpoint_vote_with_test_rpc(
                    EthereumCheckpointVoteSignOptions {
                        data_dir: root,
                        checkpoint_file: candidate,
                        ethereum_rpc: String::new(),
                        validator: "validator-3".to_string(),
                        validator_key_file: key_file,
                        vote_file: output.clone(),
                    },
                    controller_code,
                    wrapped_code,
                    candidate_checkpoint.block_hash,
                    candidate_checkpoint.receipts_root,
                    candidate_checkpoint.block_number,
                    candidate_checkpoint.observed_head_number,
                );
                (result, output)
            })
        })
        .collect::<Vec<_>>();
        let outcomes = contenders
            .into_iter()
            .map(|contender| contender.join().expect("checkpoint signing contender"))
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes.iter().filter(|(result, _)| result.is_ok()).count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|(_, output)| output.exists())
                .count(),
            1
        );

        let certificate_file = root.join("certificate.json");
        let certificate = assemble_ethereum_checkpoint_certificate(
            EthereumCheckpointCertificateAssembleOptions {
                data_dir: root.clone(),
                checkpoint_file: checkpoint_file.clone(),
                vote_files: vote_files.clone(),
                certificate_file: certificate_file.clone(),
            },
        )
        .expect("assemble exact quorum");
        verify_ethereum_checkpoint_certificate(&committee, &certificate)
            .expect("assembled certificate verifies");
        assert!(certificate_file.is_file());

        let minority_output = root.join("minority-certificate.json");
        let minority = assemble_ethereum_checkpoint_certificate(
            EthereumCheckpointCertificateAssembleOptions {
                data_dir: root.clone(),
                checkpoint_file: checkpoint_file.clone(),
                vote_files: vote_files[..2].to_vec(),
                certificate_file: minority_output.clone(),
            },
        );
        assert!(minority.is_err());
        assert!(!minority_output.exists());

        let mut wrong_chain = checkpoint;
        wrong_chain.ethereum_chain_id = 2;
        let wrong_chain_file = root.join("wrong-chain.json");
        write_json(&wrong_chain_file, &wrong_chain).expect("write wrong-chain checkpoint");
        let rejected_vote = root.join("wrong-chain.vote.json");
        let rejection = sign_ethereum_checkpoint_vote(EthereumCheckpointVoteSignOptions {
            data_dir: root.clone(),
            checkpoint_file: wrong_chain_file,
            ethereum_rpc: "http://127.0.0.1:1".to_string(),
            validator: "validator-0".to_string(),
            validator_key_file: root.join("validator-0.key.json"),
            vote_file: rejected_vote.clone(),
        });
        assert!(rejection.is_err());
        assert!(!rejected_vote.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn production_checkpoint_artifacts_drive_bidirectional_bridge_and_refund_alternate() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-ethereum-checkpoint-bridge-e2e-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create bridge E2E dir");

        let genesis = Genesis::new("postfiat-ethereum-checkpoint-bridge-e2e");
        let issuer_key = ml_dsa_65_keygen_from_seed(&[0x31; 32]);
        let operator_key = ml_dsa_65_keygen_from_seed(&[0x32; 32]);
        let issuer = address_from_public_key(&issuer_key.public_key);
        let operator = address_from_public_key(&operator_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                100_000,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                operator.clone(),
                100_000,
                Some(bytes_to_hex(&operator_key.public_key)),
            ),
        ]);

        let mut settlement_asset =
            AssetDefinition::new(&genesis.chain_id, issuer.clone(), "PUSDC", 1, 6)
                .expect("settlement asset");
        settlement_asset.max_supply = Some(1_000_000);
        let mut native_asset =
            AssetDefinition::new(&genesis.chain_id, issuer.clone(), "A651", 1, 6)
                .expect("native NAV asset");
        native_asset.max_supply = Some(1_000_000);
        let settlement_asset_id = settlement_asset.asset_id.clone();
        let native_asset_id = native_asset.asset_id.clone();
        ledger.asset_definitions = vec![settlement_asset, native_asset];

        let mut nav_asset = NavTrackedAsset::new(
            native_asset_id.clone(),
            issuer.clone(),
            operator.clone(),
            "bridge-e2e-profile",
            "USDC",
            issuer.clone(),
        )
        .expect("NAV asset");
        nav_asset.finalized_epoch = 7;
        nav_asset.nav_per_unit = 7_000_000;
        nav_asset.finalized_reserve_packet_hash = "55".repeat(48);
        nav_asset.finalized_at_height = 1;
        let settlement_nav_asset = NavTrackedAsset::new(
            settlement_asset_id.clone(),
            issuer.clone(),
            issuer.clone(),
            "bridge-e2e-settlement-profile",
            "USDC",
            issuer.clone(),
        )
        .expect("settlement NAV asset");
        ledger.nav_assets = vec![settlement_nav_asset, nav_asset];

        let mut settlement_line =
            TrustLine::new(&operator, &issuer, &settlement_asset_id, 1_000, 10)
                .expect("settlement trustline");
        settlement_line.balance = 350;
        let native_line = TrustLine::new(&operator, &issuer, &native_asset_id, 1_000, 10)
            .expect("native trustline");
        ledger.trustlines = vec![settlement_line, native_line];

        let authority_keys = (0_u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[0x81 + index; 32]))
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: genesis.chain_id.clone(),
                    genesis_hash: FastSwapOpaqueHashV1(
                        exact_hex("genesis hash", &postfiat_execution::genesis_hash(&genesis))
                            .expect("genesis hash bytes"),
                    ),
                    protocol_version: genesis.protocol_version,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 19,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 4,
                quorum: 3,
            },
            validators: authority_keys
                .iter()
                .enumerate()
                .map(|(index, key)| FastSwapValidatorV1 {
                    validator_id: format!("bridge-validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        ledger.fastswap_committees.push(committee.clone());

        let controller_code = vec![0x60, 0x11, 0x60, 0x00];
        let wrapped_code = vec![0x60, 0x12, 0x60, 0x00];
        let policy = EthereumRouteVerificationPolicyV1 {
            authority_epoch: committee.domain.committee_epoch,
            committee_root: committee.domain.committee_root,
            minimum_confirmations: 12,
            handoff_controller_code_hash: Keccak256::digest(&controller_code).into(),
            wrapped_navcoin_code_hash: Keccak256::digest(&wrapped_code).into(),
        };
        let route_id = "pftl-uniswap-production-e2e";
        ledger
            .pftl_uniswap_routes
            .push(PftlUniswapConsensusRouteState {
                route_id: route_id.to_string(),
                route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
                route_config_digest: "14".repeat(48),
                route_trust_class: ROUTE_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
                native_nav_asset_id: native_asset_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                handoff_controller: format!("0x{}", "33".repeat(20)),
                settlement_adapter: format!("0x{}", "34".repeat(20)),
                wrapped_navcoin_token: format!("0x{}", "35".repeat(20)),
                ethereum_chain_id: 1,
                route_supply_cap_atoms: 1_000,
                packet_notional_cap_atoms: 100,
                latest_finalized_nav_epoch: 7,
                return_finality_blocks: 12,
                ethereum_verification_policy: Some(policy),
                authorized_valid_supply_atoms: 0,
                pftl_spendable_supply_atoms: 0,
                native_spendable_balances_atoms: BTreeMap::new(),
                ethereum_spendable_supply_atoms: 0,
                other_registered_venue_supply_atoms: 0,
                outstanding_bridge_claims_atoms: 0,
                pending_return_import_claims_atoms: 0,
                settlement_reserve_atoms: 0,
                primary_subscription_nonces: BTreeMap::new(),
                export_packets: BTreeMap::new(),
                export_nonces: BTreeMap::new(),
                return_imports: BTreeMap::new(),
                paused: false,
            });
        ledger
            .validate_asset_state(&genesis.chain_id)
            .expect("initial E2E asset state");
        ledger
            .validate_nav_state(&genesis.chain_id)
            .expect("initial E2E NAV state");

        let subscribe = signed_bridge_asset_transaction(
            &genesis,
            &ledger,
            &operator,
            &operator_key.public_key,
            &operator_key.private_key,
            PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapPrimarySubscribe(
                PftlUniswapPrimarySubscribeOperation {
                    subscriber: operator.clone(),
                    route_id: route_id.to_string(),
                    settlement_asset_id: settlement_asset_id.clone(),
                    subscription_nonce: "61".repeat(32),
                    settlement_value_atoms: 350,
                    nav_price_settlement_atoms_per_nav_atom: 7,
                    pricing_nav_epoch: 7,
                    pricing_reserve_packet_hash: "55".repeat(48),
                },
            ),
        );
        let receipt =
            postfiat_execution::execute_asset_transaction(&genesis, &mut ledger, &subscribe, 10);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "accepted");

        let consumed_packet_hash = "62".repeat(48);
        let consumed_packet_digest = [0x63; 32];
        let consumed_export = signed_bridge_asset_transaction(
            &genesis,
            &ledger,
            &operator,
            &operator_key.public_key,
            &operator_key.private_key,
            PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapExportDebit(PftlUniswapExportDebitOperation {
                owner: operator.clone(),
                route_id: route_id.to_string(),
                packet_hash: consumed_packet_hash.clone(),
                export_nonce: "64".repeat(32),
                ethereum_recipient: format!("0x{}", "44".repeat(20)),
                amount_atoms: 40,
                destination_deadline_seconds: 1_800,
                refund_delay_blocks: 3,
                ethereum_packet_digest: Some(bytes_to_hex(&consumed_packet_digest)),
                ethereum_packet_schema_version: Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1),
            }),
        );
        let receipt = postfiat_execution::execute_asset_transaction(
            &genesis,
            &mut ledger,
            &consumed_export,
            11,
        );
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "accepted");

        let refunded_packet_hash = "65".repeat(48);
        let refunded_packet_digest = [0x66; 32];
        let refunded_export = signed_bridge_asset_transaction(
            &genesis,
            &ledger,
            &operator,
            &operator_key.public_key,
            &operator_key.private_key,
            PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapExportDebit(PftlUniswapExportDebitOperation {
                owner: operator.clone(),
                route_id: route_id.to_string(),
                packet_hash: refunded_packet_hash.clone(),
                export_nonce: "67".repeat(32),
                ethereum_recipient: format!("0x{}", "45".repeat(20)),
                amount_atoms: 10,
                destination_deadline_seconds: 1_800,
                refund_delay_blocks: 3,
                ethereum_packet_digest: Some(bytes_to_hex(&refunded_packet_digest)),
                ethereum_packet_schema_version: Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1),
            }),
        );
        let receipt = postfiat_execution::execute_asset_transaction(
            &genesis,
            &mut ledger,
            &refunded_export,
            12,
        );
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "accepted");
        assert_bridge_supply(&ledger, route_id, 50);

        let store = NodeStore::new(&root);
        store.write_genesis(&genesis).expect("write E2E genesis");
        store.write_ledger(&ledger).expect("write E2E ledger");

        let controller = [0x33; 20];
        let mut recipient_topic = [0_u8; 32];
        recipient_topic[12..].copy_from_slice(&[0x44; 20]);
        let source_packet_commitment = postfiat_bridge::ethereum_keccak256(
            &hex_to_bytes(&consumed_packet_hash).expect("packet hash bytes"),
        );
        let consume_signature = postfiat_bridge::ethereum_keccak256(
            b"PacketConsumed(bytes32,bytes32,address,bytes32,bytes32,bytes32,uint256,uint256)",
        );
        let mut consume_data = postfiat_bridge::ethereum_keccak256(&[0x14; 48]).to_vec();
        consume_data.extend_from_slice(&[0x68; 32]);
        consume_data.extend_from_slice(&postfiat_bridge::ethereum_keccak256(
            ROUTE_TRUST_CLASS_BFT_CHECKPOINT.as_bytes(),
        ));
        consume_data.extend_from_slice(&test_abi_u64(40));
        consume_data.extend_from_slice(&test_abi_u64(350));
        let (consume_root, consume_proof) = production_test_ethereum_receipt_proof(
            &root,
            route_id,
            100,
            [0x69; 32],
            [0x6a; 32],
            controller,
            &[
                consume_signature,
                consumed_packet_digest,
                source_packet_commitment,
                recipient_topic,
            ],
            &consume_data,
        );

        write_bridge_authority_keys(&root, &committee, &authority_keys);
        let consume_certificate = production_test_checkpoint_certificate(
            &root,
            route_id,
            &committee,
            &controller_code,
            &wrapped_code,
            100,
            112,
            [0x69; 32],
            consume_root,
            "consume",
        );
        let consume = signed_bridge_asset_transaction(
            &genesis,
            &ledger,
            &operator,
            &operator_key.public_key,
            &operator_key.private_key,
            PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapDestinationConsume(
                PftlUniswapDestinationConsumeOperation {
                    operator: operator.clone(),
                    route_id: route_id.to_string(),
                    packet_hash: consumed_packet_hash.clone(),
                    ethereum_consume_tx_hash: "6a".repeat(32),
                    consumed_height: 100,
                    finalized_height: 112,
                    external_event_proof: Some(EthereumExternalEventProofV1 {
                        checkpoint_certificate: consume_certificate,
                        receipt_proof: consume_proof,
                        log_index: 0,
                    }),
                },
            ),
        );
        let receipt =
            postfiat_execution::execute_asset_transaction(&genesis, &mut ledger, &consume, 13);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "accepted");
        assert_bridge_supply(&ledger, route_id, 50);
        let route = ledger
            .pftl_uniswap_route(route_id)
            .expect("route after consume");
        assert_eq!(route.ethereum_spendable_supply_atoms, 40);
        assert_eq!(route.outstanding_bridge_claims_atoms, 10);

        let return_sender = [0x71; 20];
        let return_nonce = [0x72; 32];
        let return_amount = 17;
        let return_burn_height = 120;
        let return_finalized_height = 132;
        let return_sender_text = format!("0x{}", bytes_to_hex(&return_sender));
        let return_nonce_text = bytes_to_hex(&return_nonce);
        let return_id = pftl_uniswap_return_burn_id_from_fields(
            1,
            &format!("0x{}", "33".repeat(20)),
            &format!("0x{}", "35".repeat(20)),
            &native_asset_id,
            &return_sender_text,
            &operator,
            return_amount,
            &return_nonce_text,
            return_burn_height,
        )
        .expect("return burn id");
        let return_id_bytes: [u8; 32] = hex_to_bytes(&return_id)
            .expect("return id bytes")
            .try_into()
            .expect("return id length");
        let recipient_tail = test_abi_dynamic(operator.as_bytes());
        let asset_tail =
            test_abi_dynamic(&hex_to_bytes(&native_asset_id).expect("native asset bytes"));
        let mut return_data = test_abi_u64(7 * 32).to_vec();
        return_data.extend_from_slice(&test_abi_u64(
            u64::try_from(7 * 32 + recipient_tail.len()).expect("return ABI offset"),
        ));
        return_data.extend_from_slice(&test_abi_u64(return_amount));
        return_data.extend_from_slice(&test_abi_u64(1));
        return_data.extend_from_slice(&test_abi_address(controller));
        return_data.extend_from_slice(&test_abi_address([0x35; 20]));
        return_data.extend_from_slice(&test_abi_u64(return_burn_height));
        return_data.extend_from_slice(&recipient_tail);
        return_data.extend_from_slice(&asset_tail);
        let return_signature = postfiat_bridge::ethereum_keccak256(
            b"ReturnBurned(bytes32,address,bytes32,string,bytes,uint256,uint256,address,address,uint256)",
        );
        store
            .write_ledger(&ledger)
            .expect("persist consumed ledger");
        let (return_root, return_proof) = production_test_ethereum_receipt_proof(
            &root,
            route_id,
            return_burn_height,
            [0x73; 32],
            [0x76; 32],
            controller,
            &[
                return_signature,
                return_id_bytes,
                test_abi_address(return_sender),
                return_nonce,
            ],
            &return_data,
        );
        let return_certificate = production_test_checkpoint_certificate(
            &root,
            route_id,
            &committee,
            &controller_code,
            &wrapped_code,
            return_burn_height,
            return_finalized_height,
            [0x73; 32],
            return_root,
            "return",
        );
        let return_import = signed_bridge_asset_transaction(
            &genesis,
            &ledger,
            &operator,
            &operator_key.public_key,
            &operator_key.private_key,
            PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapReturnImport(PftlUniswapReturnImportOperation {
                operator: operator.clone(),
                route_id: route_id.to_string(),
                burn_event_hash: return_id.clone(),
                ethereum_chain_id: 1,
                bridge_controller: format!("0x{}", "33".repeat(20)),
                wrapped_navcoin_token: format!("0x{}", "35".repeat(20)),
                native_nav_asset_id: native_asset_id.clone(),
                ethereum_sender: return_sender_text,
                pftl_recipient: operator.clone(),
                amount_atoms: return_amount,
                return_nonce: return_nonce_text,
                burn_height: return_burn_height,
                finalized_height: return_finalized_height,
                external_event_proof: Some(EthereumExternalEventProofV1 {
                    checkpoint_certificate: return_certificate,
                    receipt_proof: return_proof,
                    log_index: 0,
                }),
            }),
        );
        let receipt = postfiat_execution::execute_asset_transaction(
            &genesis,
            &mut ledger,
            &return_import,
            14,
        );
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "accepted");
        assert_bridge_supply(&ledger, route_id, 50);
        let route = ledger
            .pftl_uniswap_route(route_id)
            .expect("route after return");
        assert_eq!(route.ethereum_spendable_supply_atoms, 23);
        assert_eq!(route.pftl_spendable_supply_atoms, 17);

        let refund_source_commitment = postfiat_bridge::ethereum_keccak256(
            &hex_to_bytes(&refunded_packet_hash).expect("refund packet hash bytes"),
        );
        let cancel_signature = postfiat_bridge::ethereum_keccak256(
            b"PacketCancelled(bytes32,bytes32,bytes32,uint64,uint64)",
        );
        let mut cancel_data = test_abi_u64(1_800).to_vec();
        cancel_data.extend_from_slice(&test_abi_u64(1_801));
        store
            .write_ledger(&ledger)
            .expect("persist returned ledger");
        let (cancel_root, cancel_proof) = production_test_ethereum_receipt_proof(
            &root,
            route_id,
            140,
            [0x75; 32],
            [0x77; 32],
            controller,
            &[
                cancel_signature,
                refunded_packet_digest,
                refund_source_commitment,
                [0x74; 32],
            ],
            &cancel_data,
        );
        let cancel_certificate = production_test_checkpoint_certificate(
            &root,
            route_id,
            &committee,
            &controller_code,
            &wrapped_code,
            140,
            152,
            [0x75; 32],
            cancel_root,
            "cancel",
        );
        let refund_block_height = 20;
        let refund_not_before_height = ledger
            .pftl_uniswap_route(route_id)
            .and_then(|route| route.export_packets.get(&refunded_packet_hash))
            .expect("refunded packet before refund")
            .refund_not_before_height;
        let refund = signed_bridge_asset_transaction(
            &genesis,
            &ledger,
            &operator,
            &operator_key.public_key,
            &operator_key.private_key,
            PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapRefundSource(PftlUniswapRefundSourceOperation {
                operator: operator.clone(),
                route_id: route_id.to_string(),
                packet_hash: refunded_packet_hash.clone(),
                non_consumption_proof_hash: pftl_uniswap_non_consumption_proof_hash(
                    route_id,
                    &refunded_packet_hash,
                    refund_not_before_height,
                )
                .expect("refund audit commitment"),
                external_event_proof: Some(EthereumExternalEventProofV1 {
                    checkpoint_certificate: cancel_certificate,
                    receipt_proof: cancel_proof,
                    log_index: 0,
                }),
            }),
        );
        let receipt = postfiat_execution::execute_asset_transaction(
            &genesis,
            &mut ledger,
            &refund,
            refund_block_height,
        );
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "accepted");
        assert_bridge_supply(&ledger, route_id, 50);
        let route = ledger
            .pftl_uniswap_route(route_id)
            .expect("route after refund alternate");
        assert_eq!(route.outstanding_bridge_claims_atoms, 0);
        assert_eq!(route.pftl_spendable_supply_atoms, 27);
        assert_eq!(route.ethereum_spendable_supply_atoms, 23);
        assert_eq!(
            route
                .export_packets
                .get(&consumed_packet_hash)
                .expect("consumed packet")
                .status,
            postfiat_types::PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED
        );
        assert_eq!(
            route
                .export_packets
                .get(&refunded_packet_hash)
                .expect("refunded packet")
                .status,
            postfiat_types::PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED
        );
        assert!(ledger.pftl_uniswap_receipts.iter().all(|receipt| {
            matches!(
                receipt.transition.as_str(),
                "primary_subscription"
                    | "export_debit"
                    | "destination_consume"
                    | "return_imported"
                    | "source_refunded"
            )
        }));

        fs::remove_dir_all(root).expect("remove bridge E2E dir");
    }

    fn signed_bridge_asset_transaction(
        genesis: &Genesis,
        ledger: &LedgerState,
        source: &str,
        public_key: &[u8],
        private_key: &[u8],
        transaction_kind: &str,
        operation: AssetTransactionOperation,
    ) -> SignedAssetTransaction {
        let sequence = ledger
            .account(source)
            .expect("bridge transaction source")
            .sequence
            .checked_add(1)
            .expect("bridge transaction sequence");
        let mut fee = postfiat_execution::MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let unsigned = UnsignedAssetTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: postfiat_execution::genesis_hash(genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: transaction_kind.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: source.to_string(),
                fee,
                sequence,
                operation: operation.clone(),
            };
            let signed = SignedAssetTransaction {
                signature_hex: bytes_to_hex(
                    &ml_dsa_65_sign(private_key, &unsigned.signing_bytes())
                        .expect("sign bridge asset transaction"),
                ),
                public_key_hex: bytes_to_hex(public_key),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                unsigned,
            };
            let minimum =
                postfiat_execution::minimum_asset_transaction_fee_for_ledger(ledger, &signed);
            if fee >= minimum {
                return signed;
            }
            fee = minimum;
        }
        panic!("bridge asset transaction fee did not converge");
    }

    fn assert_bridge_supply(ledger: &LedgerState, route_id: &str, expected: u64) {
        let route = ledger.pftl_uniswap_route(route_id).expect("bridge route");
        let route_total = route
            .pftl_spendable_supply_atoms
            .checked_add(route.outstanding_bridge_claims_atoms)
            .and_then(|value| value.checked_add(route.pending_return_import_claims_atoms))
            .and_then(|value| value.checked_add(route.ethereum_spendable_supply_atoms))
            .and_then(|value| value.checked_add(route.other_registered_venue_supply_atoms))
            .expect("bridge route supply sum");
        assert_eq!(route_total, expected);
        assert_eq!(route.authorized_valid_supply_atoms, expected);
        assert_eq!(
            postfiat_execution::issued_asset_supply(ledger, &route.native_nav_asset_id)
                .expect("global issued asset supply"),
            expected
        );
    }

    fn write_bridge_authority_keys(
        root: &Path,
        committee: &FastSwapCommitteeV1,
        keys: &[MlDsa65KeyPair],
    ) {
        for (validator, key) in committee.validators.iter().zip(keys) {
            let key_file = root.join(format!("{}.key.json", validator.validator_id));
            write_json(
                &key_file,
                &ValidatorKeyFile {
                    validators: vec![ValidatorKeyRecord {
                        node_id: validator.validator_id.clone(),
                        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                        public_key_hex: bytes_to_hex(&key.public_key),
                        private_key_hex: bytes_to_hex(&key.private_key),
                    }],
                },
            )
            .expect("write bridge authority key");
            crate::set_private_file_permissions(&key_file).expect("protect bridge authority key");
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn production_test_checkpoint_certificate(
        root: &Path,
        route_id: &str,
        committee: &FastSwapCommitteeV1,
        controller_code: &[u8],
        wrapped_code: &[u8],
        block_number: u64,
        head_number: u64,
        block_hash: [u8; 32],
        receipts_root: [u8; 32],
        label: &str,
    ) -> EthereumCheckpointCertificateV1 {
        let checkpoint_file = root.join(format!("{label}.checkpoint.json"));
        let (observe_rpc, observe_thread) = spawn_test_ethereum_rpc(
            controller_code.to_vec(),
            wrapped_code.to_vec(),
            block_hash,
            receipts_root,
            block_number,
            head_number,
            5,
        );
        let checkpoint = observe_ethereum_checkpoint(EthereumCheckpointObserveOptions {
            data_dir: root.to_path_buf(),
            route_id: route_id.to_string(),
            ethereum_rpc: observe_rpc,
            block_number: Some(block_number),
            checkpoint_file: checkpoint_file.clone(),
        })
        .expect("observe production bridge checkpoint");
        observe_thread
            .join()
            .expect("bridge checkpoint observer RPC");
        assert_eq!(checkpoint.block_number, block_number);
        assert_eq!(checkpoint.receipts_root, receipts_root);

        let mut vote_files = Vec::new();
        for validator in committee
            .validators
            .iter()
            .take(usize::from(committee.domain.quorum))
        {
            let vote_file = root.join(format!("{label}.{}.vote.json", validator.validator_id));
            sign_checkpoint_vote_with_test_rpc(
                EthereumCheckpointVoteSignOptions {
                    data_dir: root.to_path_buf(),
                    checkpoint_file: checkpoint_file.clone(),
                    ethereum_rpc: String::new(),
                    validator: validator.validator_id.clone(),
                    validator_key_file: root.join(format!("{}.key.json", validator.validator_id)),
                    vote_file: vote_file.clone(),
                },
                controller_code.to_vec(),
                wrapped_code.to_vec(),
                block_hash,
                receipts_root,
                block_number,
                head_number,
            )
            .expect("sign production bridge checkpoint vote");
            vote_files.push(vote_file);
        }
        let certificate_file = root.join(format!("{label}.certificate.json"));
        let certificate = assemble_ethereum_checkpoint_certificate(
            EthereumCheckpointCertificateAssembleOptions {
                data_dir: root.to_path_buf(),
                checkpoint_file,
                vote_files,
                certificate_file: certificate_file.clone(),
            },
        )
        .expect("assemble production bridge checkpoint certificate");
        assert!(certificate_file.is_file());
        certificate
    }

    fn test_rlp_bytes(bytes: &[u8]) -> Vec<u8> {
        if bytes.len() == 1 && bytes[0] <= 0x7f {
            return bytes.to_vec();
        }
        if bytes.len() < 56 {
            let mut encoded = vec![0x80 + bytes.len() as u8];
            encoded.extend_from_slice(bytes);
            return encoded;
        }
        let length_bytes = bytes.len().to_be_bytes();
        let first = length_bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(length_bytes.len() - 1);
        let length = &length_bytes[first..];
        let mut encoded = vec![0xb7 + length.len() as u8];
        encoded.extend_from_slice(length);
        encoded.extend_from_slice(bytes);
        encoded
    }

    fn test_rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
        let payload = items.concat();
        if payload.len() < 56 {
            let mut encoded = vec![0xc0 + payload.len() as u8];
            encoded.extend_from_slice(&payload);
            return encoded;
        }
        let length_bytes = payload.len().to_be_bytes();
        let first = length_bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(length_bytes.len() - 1);
        let length = &length_bytes[first..];
        let mut encoded = vec![0xf7 + length.len() as u8];
        encoded.extend_from_slice(length);
        encoded.extend_from_slice(&payload);
        encoded
    }

    fn test_abi_u64(value: u64) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn test_abi_address(value: [u8; 20]) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[12..].copy_from_slice(&value);
        word
    }

    fn test_abi_dynamic(value: &[u8]) -> Vec<u8> {
        let mut encoded =
            test_abi_u64(u64::try_from(value.len()).expect("test ABI length")).to_vec();
        encoded.extend_from_slice(value);
        encoded.resize(encoded.len().div_ceil(32) * 32, 0);
        encoded
    }

    fn test_ethereum_receipt_proof(
        emitter: [u8; 20],
        topics: &[[u8; 32]],
        data: &[u8],
    ) -> ([u8; 32], EthereumReceiptProofV1) {
        let topics = topics
            .iter()
            .map(|topic| test_rlp_bytes(topic))
            .collect::<Vec<_>>();
        let log = test_rlp_list(&[
            test_rlp_bytes(&emitter),
            test_rlp_list(&topics),
            test_rlp_bytes(data),
        ]);
        let receipt = test_rlp_list(&[
            test_rlp_bytes(&[1]),
            test_rlp_bytes(&[1]),
            test_rlp_bytes(&[0; 256]),
            test_rlp_list(&[log]),
        ]);
        let leaf = test_rlp_list(&[test_rlp_bytes(&[0x20, 0x80]), test_rlp_bytes(&receipt)]);
        let root = postfiat_bridge::ethereum_keccak256(&leaf);
        (
            root,
            EthereumReceiptProofV1 {
                transaction_index: 0,
                receipt_rlp: receipt,
                proof_nodes_rlp: vec![leaf],
            },
        )
    }

    fn production_test_ethereum_receipt_proof(
        root: &Path,
        route_id: &str,
        block_number: u64,
        block_hash: [u8; 32],
        transaction_hash: [u8; 32],
        emitter: [u8; 20],
        topics: &[[u8; 32]],
        data: &[u8],
    ) -> ([u8; 32], EthereumReceiptProofV1) {
        let (receipts_root, expected_proof) = test_ethereum_receipt_proof(emitter, topics, data);
        let transaction_hash_text = format!("0x{}", bytes_to_hex(&transaction_hash));
        let block_hash_text = format!("0x{}", bytes_to_hex(&block_hash));
        let receipt = serde_json::json!({
            "type": "0x0",
            "status": "0x1",
            "cumulativeGasUsed": "0x1",
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "logs": [{
                "address": format!("0x{}", bytes_to_hex(&emitter)),
                "topics": topics
                    .iter()
                    .map(|topic| format!("0x{}", bytes_to_hex(topic)))
                    .collect::<Vec<_>>(),
                "data": format!("0x{}", bytes_to_hex(data)),
            }],
            "transactionHash": transaction_hash_text,
            "blockHash": block_hash_text,
            "blockNumber": format!("0x{block_number:x}"),
            "transactionIndex": "0x0",
        });
        let block = serde_json::json!({
            "number": format!("0x{block_number:x}"),
            "hash": block_hash_text,
            "receiptsRoot": format!("0x{}", bytes_to_hex(&receipts_root)),
            "transactions": [transaction_hash_text],
        });
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind production receipt builder test RPC");
        let address = listener
            .local_addr()
            .expect("production receipt builder test RPC address");
        let receipt_for_server = receipt.clone();
        let server = std::thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener
                    .accept()
                    .expect("accept production receipt builder RPC request");
                let request = read_test_http_request(&mut stream);
                let result = match request
                    .get("method")
                    .and_then(Value::as_str)
                    .expect("production receipt builder RPC method")
                {
                    "eth_chainId" => Value::String("0x1".to_string()),
                    "eth_getTransactionReceipt" => receipt_for_server.clone(),
                    "eth_getBlockByHash" => block.clone(),
                    "eth_getBlockReceipts" => Value::Array(vec![receipt_for_server.clone()]),
                    method => panic!("unexpected production receipt builder RPC method {method}"),
                };
                let body = serde_json::to_vec(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": result,
                }))
                .expect("serialize production receipt builder RPC response");
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("write production receipt builder RPC headers");
                stream
                    .write_all(&body)
                    .expect("write production receipt builder RPC body");
            }
        });
        let proof_file = root.join(format!(
            "receipt-proof-{}-{}.json",
            block_number,
            bytes_to_hex(&transaction_hash)
        ));
        let artifact = build_ethereum_receipt_proof(EthereumReceiptProofBuildOptions {
            data_dir: root.to_path_buf(),
            route_id: route_id.to_string(),
            ethereum_rpc: format!("http://{address}"),
            transaction_hash: format!("0x{}", bytes_to_hex(&transaction_hash)),
            proof_file: proof_file.clone(),
        })
        .expect("build production Ethereum receipt proof");
        server
            .join()
            .expect("production receipt builder test RPC server");
        assert!(proof_file.is_file());
        assert_eq!(
            artifact.receipts_root,
            format!("0x{}", bytes_to_hex(&receipts_root))
        );
        assert_eq!(artifact.proof.receipt_rlp, expected_proof.receipt_rlp);
        postfiat_bridge::verify_ethereum_receipt_log(receipts_root, &artifact.proof, 0)
            .expect("production-built receipt proof verifies");
        (receipts_root, artifact.proof)
    }

    fn spawn_test_ethereum_rpc(
        controller_code: Vec<u8>,
        wrapped_code: Vec<u8>,
        block_hash: [u8; 32],
        receipts_root: [u8; 32],
        block_number: u64,
        head_number: u64,
        expected_requests: usize,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind Ethereum RPC test server");
        let address = listener.local_addr().expect("Ethereum RPC test address");
        let handle = std::thread::spawn(move || {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().expect("accept Ethereum RPC request");
                let request = read_test_http_request(&mut stream);
                let method = request
                    .get("method")
                    .and_then(Value::as_str)
                    .expect("JSON-RPC method");
                let result = match method {
                    "eth_chainId" => Value::String("0x1".to_string()),
                    "eth_blockNumber" => Value::String(format!("0x{head_number:x}")),
                    "eth_getBlockByNumber" => serde_json::json!({
                        "number": format!("0x{block_number:x}"),
                        "hash": format!("0x{}", bytes_to_hex(&block_hash)),
                        "receiptsRoot": format!("0x{}", bytes_to_hex(&receipts_root)),
                    }),
                    "eth_getCode" => {
                        let address = request
                            .get("params")
                            .and_then(Value::as_array)
                            .and_then(|params| params.first())
                            .and_then(Value::as_str)
                            .expect("eth_getCode address");
                        let code = if address.ends_with(&"33".repeat(20)) {
                            &controller_code
                        } else {
                            &wrapped_code
                        };
                        Value::String(format!("0x{}", bytes_to_hex(code)))
                    }
                    other => panic!("unexpected Ethereum RPC method {other}"),
                };
                let body = serde_json::to_vec(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": result,
                }))
                .expect("serialize Ethereum RPC response");
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("write Ethereum RPC response headers");
                stream
                    .write_all(&body)
                    .expect("write Ethereum RPC response body");
            }
        });
        (format!("http://{address}"), handle)
    }

    fn sign_checkpoint_vote_with_test_rpc(
        mut options: EthereumCheckpointVoteSignOptions,
        controller_code: Vec<u8>,
        wrapped_code: Vec<u8>,
        block_hash: [u8; 32],
        receipts_root: [u8; 32],
        block_number: u64,
        head_number: u64,
    ) -> io::Result<EthereumCheckpointVoteV1> {
        let (ethereum_rpc, rpc_thread) = spawn_test_ethereum_rpc(
            controller_code,
            wrapped_code,
            block_hash,
            receipts_root,
            block_number,
            head_number,
            5,
        );
        options.ethereum_rpc = ethereum_rpc;
        let result = sign_ethereum_checkpoint_vote(options);
        rpc_thread.join().expect("Ethereum RPC test server");
        result
    }

    fn read_test_http_request(stream: &mut TcpStream) -> Value {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set test RPC timeout");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 2048];
        loop {
            let count = stream.read(&mut chunk).expect("read Ethereum RPC request");
            assert!(count > 0, "Ethereum RPC request closed before body");
            request.extend_from_slice(&chunk[..count]);
            let Some(header_index) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                continue;
            };
            let body_start = header_index + 4;
            let headers = std::str::from_utf8(&request[..body_start]).expect("request headers");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .expect("request Content-Length");
            if request.len() >= body_start + content_length {
                return serde_json::from_slice(&request[body_start..body_start + content_length])
                    .expect("parse Ethereum RPC request");
            }
        }
    }
}
