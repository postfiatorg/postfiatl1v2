//! The Base checkpoint source of a genesis external lane: design
//! `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md` sections 4
//! (b), 4.1, 7 and 9. A committee member observes a finalized Base block as a
//! lane checkpoint, signs one vote for it after re-observing the block from
//! its own Base RPC, quorum votes form the certificate, and the receipt proof
//! of a `Locked` event under that checkpoint becomes an
//! `external_lane_release_v1` operation.
//!
//! The checkpoint is the `EthereumFinalizedCheckpointV1` that
//! `verify_external_lane_lock` reads: its route fields carry the lane id, the
//! lane's route-config digest, the lock contract and the token with their
//! runtime code hashes, and it names the lane's active committee.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use postfiat_bridge::{
    decode_locked_event, ethereum_keccak256, locked_event_topic,
    verify_ethereum_checkpoint_certificate, verify_ethereum_receipt_log, EthereumLogV1,
};
use postfiat_crypto_provider::{
    hex_to_bytes, ml_dsa_65_sign_with_context, ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM,
};
use postfiat_storage::NodeStore;
use postfiat_types::{
    AssetTransactionOperation, EthereumCheckpointCertificateV1, EthereumCheckpointVoteV1,
    EthereumExternalEventProofV1, EthereumFinalizedCheckpointV1, ExternalLaneReleaseOperationV1,
    FastSwapChainDomainV1, FastSwapCommitteeV1, FastSwapOpaqueHashV1, GenesisExternalLane,
    ETHEREUM_CHECKPOINT_SCHEMA_V1, ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::ethereum_checkpoint_signing::{
    ethereum_checkpoint_signing_state_path, ethereum_rpc_call, ethereum_rpc_hex_bytes,
    exact_evm_address, exact_hex, invalid_data, load_or_create_checkpoint_signing_state,
    rpc_hex_u64, validate_checkpoint_signing_state, value_hex_array, value_hex_u64, write_json,
    write_private_json, EthereumCheckpointSigningStateV1, EthereumRpcEndpoint,
    ETHEREUM_CHECKPOINT_SIGNING_STATE_SCHEMA_V1,
};
use crate::ethereum_receipt_proof_builder::receipt_proof_from_rpc;
use crate::{read_json_file, read_validator_key_file, validator_key_record};

const EXTERNAL_LANE_CHECKPOINT_SIGNING_STATE_DIR: &str = "external-lane-checkpoint-signing";
const TRANSFER_EVENT_SIGNATURE: &[u8] = b"Transfer(address,address,uint256)";
const RECEIPT_LOG_INDEX_INVALID: &str = "ethereum_receipt_log_index_invalid";

/// Why a Base checkpoint source command refused. Carried inside the returned
/// `io::Error`; [`external_lane_checkpoint_refusal`] reads it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLaneCheckpointRefusal {
    LaneNotConfigured,
    CommitteeMissing,
    ChainIdMismatch,
    BlockNotFinalized,
    ConfirmationDepthNotReached,
    LockCodeHashMismatch,
    TokenCodeHashMismatch,
    CheckpointNotForLane,
    ObservationMismatch,
    NotCommitteeMember,
    SigningKeyMismatch,
    ConflictingCheckpoint,
    DuplicateVote,
    VoteNotMember,
    VoteOnOtherCheckpoint,
    BelowQuorum,
    CertificateInvalid,
    ReceiptBlockMismatch,
    LockedLogMissing,
    LockedLogAmbiguous,
    LogOfAnotherContract,
    LogOfAnotherEvent,
    LockedEventInvalid,
    TokenMismatch,
    OperationInvalid,
    SigningStateInvalid,
}

impl ExternalLaneCheckpointRefusal {
    pub const fn code(self) -> &'static str {
        match self {
            Self::LaneNotConfigured => "external_lane_not_configured",
            Self::CommitteeMissing => "external_lane_committee_missing",
            Self::ChainIdMismatch => "base_chain_id_mismatch",
            Self::BlockNotFinalized => "base_block_not_finalized",
            Self::ConfirmationDepthNotReached => "base_confirmation_depth_not_reached",
            Self::LockCodeHashMismatch => "base_lock_code_hash_mismatch",
            Self::TokenCodeHashMismatch => "base_token_code_hash_mismatch",
            Self::CheckpointNotForLane => "external_lane_checkpoint_mismatch",
            Self::ObservationMismatch => "base_observation_mismatch",
            Self::NotCommitteeMember => "checkpoint_signer_not_member",
            Self::SigningKeyMismatch => "checkpoint_signing_key_mismatch",
            Self::ConflictingCheckpoint => "checkpoint_conflicting_vote",
            Self::DuplicateVote => "checkpoint_vote_duplicate",
            Self::VoteNotMember => "checkpoint_vote_not_member",
            Self::VoteOnOtherCheckpoint => "checkpoint_vote_other_checkpoint",
            Self::BelowQuorum => "checkpoint_votes_below_quorum",
            Self::CertificateInvalid => "checkpoint_certificate_invalid",
            Self::ReceiptBlockMismatch => "lock_receipt_block_mismatch",
            Self::LockedLogMissing => "lock_receipt_locked_log_missing",
            Self::LockedLogAmbiguous => "lock_receipt_locked_log_ambiguous",
            Self::LogOfAnotherContract => "lock_receipt_log_other_contract",
            Self::LogOfAnotherEvent => "lock_receipt_log_other_event",
            Self::LockedEventInvalid => "lock_receipt_locked_event_invalid",
            Self::TokenMismatch => "lock_receipt_token_mismatch",
            Self::OperationInvalid => "external_lane_release_operation_invalid",
            Self::SigningStateInvalid => "checkpoint_signing_state_invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneCheckpointError {
    pub refusal: ExternalLaneCheckpointRefusal,
    pub detail: String,
}

impl std::fmt::Display for ExternalLaneCheckpointError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.refusal.code(), self.detail)
    }
}

impl std::error::Error for ExternalLaneCheckpointError {}

/// The typed refusal inside an error of these commands, if any.
pub fn external_lane_checkpoint_refusal(
    error: &io::Error,
) -> Option<ExternalLaneCheckpointRefusal> {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ExternalLaneCheckpointError>())
        .map(|inner| inner.refusal)
}

fn refuse(refusal: ExternalLaneCheckpointRefusal, detail: impl Into<String>) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        ExternalLaneCheckpointError {
            refusal,
            detail: detail.into(),
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneCheckpointObserveOptions {
    pub data_dir: PathBuf,
    pub lane_id: String,
    pub base_rpc: String,
    pub block_number: Option<u64>,
    pub checkpoint_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneCheckpointSignOptions {
    pub data_dir: PathBuf,
    pub checkpoint_file: PathBuf,
    pub base_rpc: String,
    pub validator: String,
    pub validator_key_file: PathBuf,
    pub vote_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneCheckpointAssembleOptions {
    pub data_dir: PathBuf,
    pub checkpoint_file: PathBuf,
    pub vote_files: Vec<PathBuf>,
    pub certificate_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneLockProofOptions {
    pub data_dir: PathBuf,
    pub lane_id: String,
    pub base_rpc: String,
    pub transaction_hash: String,
    pub certificate_file: PathBuf,
    pub submitter: String,
    /// The `Locked` log's index in its receipt; `None` picks the receipt's
    /// one `Locked` log of the lane's lock contract.
    pub log_index: Option<u32>,
    pub operation_file: PathBuf,
}

/// A lane's genesis policy decoded, with the committee it names at the next
/// PFTL block height: the genesis policy's, or the one the last committed
/// committee move binds (the selector `execute_external_lane_release` uses).
struct ActiveLane {
    lane: GenesisExternalLane,
    committee: FastSwapCommitteeV1,
    route_config_digest: [u8; 48],
    lock_contract: [u8; 20],
    token: [u8; 20],
    lock_code_hash: [u8; 32],
    token_code_hash: [u8; 32],
    confirmation_depth: u32,
}

fn active_lane(data_dir: &Path, lane_id: &str) -> io::Result<ActiveLane> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let governance = store.read_governance()?;
    let height = crate::read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)?
        .height
        .saturating_add(1);
    let (lane_index, lane) = genesis
        .external_lanes()
        .iter()
        .enumerate()
        .find(|(_, lane)| lane.id == lane_id)
        .ok_or_else(|| {
            refuse(
                ExternalLaneCheckpointRefusal::LaneNotConfigured,
                format!("genesis configures no external lane `{lane_id}`"),
            )
        })?;
    let selector = governance
        .external_lane_committees(&genesis)
        .selector_at(lane_index, height);
    let chain = FastSwapChainDomainV1 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: FastSwapOpaqueHashV1(exact_hex(
            "genesis hash",
            &postfiat_execution::genesis_hash(&genesis),
        )?),
        protocol_version: genesis.protocol_version,
    };
    let committee = ledger
        .fastswap_committees
        .iter()
        .find(|committee| committee.domain.chain == chain && selector.matches(lane, committee))
        .ok_or_else(|| {
            refuse(
                ExternalLaneCheckpointRefusal::CommitteeMissing,
                format!(
                    "no registered committee matches the committee external lane `{lane_id}` names at height {height}"
                ),
            )
        })?;
    committee.validate().map_err(invalid_data)?;
    let policy = &lane.policy;
    Ok(ActiveLane {
        lane: lane.clone(),
        committee: committee.clone(),
        route_config_digest: lane.route_config_digest().map_err(invalid_data)?,
        lock_contract: exact_evm_address("lock contract", &policy.lock_contract_address)?,
        token: exact_evm_address("token", &policy.token_address)?,
        lock_code_hash: exact_hex("lock runtime code hash", &policy.lock_runtime_code_hash)?,
        token_code_hash: exact_hex("token runtime code hash", &policy.token_runtime_code_hash)?,
        confirmation_depth: u32::try_from(policy.confirmation_depth).map_err(invalid_data)?,
    })
}

impl ActiveLane {
    /// Every field of `checkpoint` except the observed block must be the
    /// lane's, and the observed head must be the lane's depth past the block.
    fn require_checkpoint(&self, checkpoint: &EthereumFinalizedCheckpointV1) -> io::Result<()> {
        checkpoint.validate().map_err(invalid_data)?;
        let domain = &self.committee.domain;
        if checkpoint.schema_version != ETHEREUM_CHECKPOINT_SCHEMA_V1
            || checkpoint.pftl_domain != domain.chain
            || checkpoint.route_id != self.lane.id
            || checkpoint.route_config_digest != FastSwapOpaqueHashV1(self.route_config_digest)
            || checkpoint.ethereum_chain_id != self.lane.policy.chain_id
            || checkpoint.minimum_confirmations != self.confirmation_depth
            || checkpoint.authority_epoch != domain.committee_epoch
            || checkpoint.committee_root != domain.committee_root
            || checkpoint.handoff_controller != self.lock_contract
            || checkpoint.wrapped_navcoin_token != self.token
            || checkpoint.handoff_controller_code_hash != self.lock_code_hash
            || checkpoint.wrapped_navcoin_code_hash != self.token_code_hash
        {
            return Err(refuse(
                ExternalLaneCheckpointRefusal::CheckpointNotForLane,
                format!(
                    "checkpoint does not bind external lane `{}` and its active committee epoch {}",
                    self.lane.id, domain.committee_epoch
                ),
            ));
        }
        self.require_depth(checkpoint.observed_head_number, checkpoint.block_number)
    }

    fn require_depth(&self, head: u64, block_number: u64) -> io::Result<()> {
        let depth = self.lane.policy.confirmation_depth;
        if head
            .checked_sub(block_number)
            .is_none_or(|distance| distance < depth)
        {
            return Err(refuse(
                ExternalLaneCheckpointRefusal::ConfirmationDepthNotReached,
                format!("Base head {head} is fewer than {depth} blocks past block {block_number}"),
            ));
        }
        Ok(())
    }

    fn require_chain_id(&self, endpoint: &EthereumRpcEndpoint) -> io::Result<()> {
        let chain_id = rpc_hex_u64(endpoint, "eth_chainId", serde_json::json!([]))?;
        if chain_id != self.lane.policy.chain_id {
            return Err(refuse(
                ExternalLaneCheckpointRefusal::ChainIdMismatch,
                format!(
                    "Base RPC chain id {chain_id} is not the lane's chain id {}",
                    self.lane.policy.chain_id
                ),
            ));
        }
        Ok(())
    }

    /// Reads the block (the `finalized` block, or `block_number` when it is
    /// at or below it), the head and both runtime codes at the block from
    /// `endpoint`, and returns the lane checkpoint of that block.
    fn observe(
        &self,
        endpoint: &EthereumRpcEndpoint,
        block_number: Option<u64>,
    ) -> io::Result<EthereumFinalizedCheckpointV1> {
        self.require_chain_id(endpoint)?;
        let finalized = rpc_block(endpoint, Value::String("finalized".to_string()))?;
        let finalized_number = value_hex_u64(&finalized, "number")?;
        let block = match block_number {
            None => finalized,
            Some(requested) if requested > finalized_number => {
                return Err(refuse(
                    ExternalLaneCheckpointRefusal::BlockNotFinalized,
                    format!(
                        "Base block {requested} is above the finalized block {finalized_number}"
                    ),
                ));
            }
            Some(requested) => {
                let block = rpc_block(endpoint, Value::String(format!("0x{requested:x}")))?;
                if value_hex_u64(&block, "number")? != requested {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Base RPC returned a block at the wrong height",
                    ));
                }
                block
            }
        };
        let number = value_hex_u64(&block, "number")?;
        if number == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Base checkpoint block is block zero",
            ));
        }
        let observed_head_number = rpc_hex_u64(endpoint, "eth_blockNumber", serde_json::json!([]))?;
        self.require_depth(observed_head_number, number)?;
        let at = format!("0x{number:x}");
        let lock_code_hash =
            runtime_code_hash(endpoint, &self.lane.policy.lock_contract_address, &at)?;
        if lock_code_hash != Some(self.lock_code_hash) {
            return Err(refuse(
                ExternalLaneCheckpointRefusal::LockCodeHashMismatch,
                format!(
                    "lock contract {} runtime code at block {number} does not hash to the lane's pinned hash",
                    self.lane.policy.lock_contract_address
                ),
            ));
        }
        let token_code_hash = runtime_code_hash(endpoint, &self.lane.policy.token_address, &at)?;
        if token_code_hash != Some(self.token_code_hash) {
            return Err(refuse(
                ExternalLaneCheckpointRefusal::TokenCodeHashMismatch,
                format!(
                    "token {} runtime code at block {number} does not hash to the lane's pinned hash",
                    self.lane.policy.token_address
                ),
            ));
        }
        let checkpoint = EthereumFinalizedCheckpointV1 {
            schema_version: ETHEREUM_CHECKPOINT_SCHEMA_V1,
            pftl_domain: self.committee.domain.chain.clone(),
            route_id: self.lane.id.clone(),
            route_config_digest: FastSwapOpaqueHashV1(self.route_config_digest),
            ethereum_chain_id: self.lane.policy.chain_id,
            block_number: number,
            block_hash: value_hex_array::<32>(&block, "hash")?,
            receipts_root: value_hex_array::<32>(&block, "receiptsRoot")?,
            observed_head_number,
            minimum_confirmations: self.confirmation_depth,
            authority_epoch: self.committee.domain.committee_epoch,
            committee_root: self.committee.domain.committee_root,
            handoff_controller: self.lock_contract,
            wrapped_navcoin_token: self.token,
            handoff_controller_code_hash: self.lock_code_hash,
            wrapped_navcoin_code_hash: self.token_code_hash,
        };
        self.require_checkpoint(&checkpoint)?;
        Ok(checkpoint)
    }
}

fn rpc_block(endpoint: &EthereumRpcEndpoint, tag: Value) -> io::Result<Value> {
    let block = ethereum_rpc_call(
        endpoint,
        "eth_getBlockByNumber",
        Value::Array(vec![tag, Value::Bool(false)]),
    )?;
    if block.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Base RPC has no such block",
        ));
    }
    Ok(block)
}

/// Keccak-256 of the account's runtime code at `at`; `None` when it has none.
fn runtime_code_hash(
    endpoint: &EthereumRpcEndpoint,
    address: &str,
    at: &str,
) -> io::Result<Option<[u8; 32]>> {
    let code = ethereum_rpc_hex_bytes(endpoint, "eth_getCode", serde_json::json!([address, at]))?;
    Ok((!code.is_empty()).then(|| ethereum_keccak256(&code)))
}

/// `external-lane-checkpoint-observe`: the lane checkpoint of a finalized
/// Base block, written to `checkpoint_file`.
pub fn observe_external_lane_checkpoint(
    options: ExternalLaneCheckpointObserveOptions,
) -> io::Result<EthereumFinalizedCheckpointV1> {
    let lane = active_lane(&options.data_dir, &options.lane_id)?;
    let endpoint = EthereumRpcEndpoint::parse(&options.base_rpc)?;
    let checkpoint = lane.observe(&endpoint, options.block_number)?;
    write_json(&options.checkpoint_file, &checkpoint)?;
    Ok(checkpoint)
}

/// `external-lane-checkpoint-sign`: one committee member's vote. The member
/// re-observes the block from its own RPC and signs only a checkpoint equal
/// to its observation; its signing state holds one checkpoint per lane,
/// committee epoch and block number, so it never signs two.
pub fn sign_external_lane_checkpoint_vote(
    options: ExternalLaneCheckpointSignOptions,
) -> io::Result<EthereumCheckpointVoteV1> {
    let checkpoint: EthereumFinalizedCheckpointV1 =
        read_json_file(&options.checkpoint_file, "external lane checkpoint")?;
    let lane = active_lane(&options.data_dir, &checkpoint.route_id)?;
    lane.require_checkpoint(&checkpoint)?;
    let member = lane
        .committee
        .validators
        .iter()
        .find(|validator| validator.validator_id == options.validator)
        .ok_or_else(|| {
            refuse(
                ExternalLaneCheckpointRefusal::NotCommitteeMember,
                format!(
                    "`{}` is not a member of the active committee of external lane `{}`",
                    options.validator, lane.lane.id
                ),
            )
        })?;
    // The loader refuses a key file whose private key does not sign for its
    // public key, and the lookup one that holds no record of this member;
    // both are this command's key mismatch.
    let unusable_key_file = |error: io::Error| {
        if error.kind() == io::ErrorKind::InvalidData {
            refuse(
                ExternalLaneCheckpointRefusal::SigningKeyMismatch,
                format!(
                    "the key file of `{}` is unusable: {error}",
                    options.validator
                ),
            )
        } else {
            error
        }
    };
    let key_file =
        read_validator_key_file(&options.validator_key_file).map_err(unusable_key_file)?;
    let key_record =
        validator_key_record(&key_file, &options.validator).map_err(unusable_key_file)?;
    if key_record.algorithm_id != ML_DSA_65_ALGORITHM
        || hex_to_bytes(&key_record.public_key_hex).map_err(invalid_data)? != member.public_key
    {
        return Err(refuse(
            ExternalLaneCheckpointRefusal::SigningKeyMismatch,
            format!(
                "the key file's key for `{}` is not its committee key",
                options.validator
            ),
        ));
    }

    let endpoint = EthereumRpcEndpoint::parse(&options.base_rpc)?;
    let observed = lane.observe(&endpoint, Some(checkpoint.block_number))?;
    if observed.observed_head_number < checkpoint.observed_head_number
        || (EthereumFinalizedCheckpointV1 {
            observed_head_number: checkpoint.observed_head_number,
            ..observed
        }) != checkpoint
    {
        return Err(refuse(
            ExternalLaneCheckpointRefusal::ObservationMismatch,
            format!(
                "this validator's Base RPC does not observe the checkpoint of block {}",
                checkpoint.block_number
            ),
        ));
    }

    let state_path = ethereum_checkpoint_signing_state_path(
        &options
            .data_dir
            .join(EXTERNAL_LANE_CHECKPOINT_SIGNING_STATE_DIR),
        &checkpoint,
        &options.validator,
    );
    let signing_state_invalid = |error: io::Error| {
        if error.kind() == io::ErrorKind::InvalidData {
            refuse(
                ExternalLaneCheckpointRefusal::SigningStateInvalid,
                format!("`{}`: {error}", state_path.display()),
            )
        } else {
            error
        }
    };
    let mut state = load_or_create_checkpoint_signing_state(
        &state_path,
        EthereumCheckpointSigningStateV1 {
            schema: ETHEREUM_CHECKPOINT_SIGNING_STATE_SCHEMA_V1.to_string(),
            validator_id: options.validator.clone(),
            checkpoint: checkpoint.clone(),
            vote: None,
        },
    )
    .map_err(signing_state_invalid)?;
    if state.checkpoint != checkpoint {
        return Err(refuse(
            ExternalLaneCheckpointRefusal::ConflictingCheckpoint,
            format!(
                "`{}` already holds a different checkpoint of external lane `{}` block {}",
                options.validator, lane.lane.id, checkpoint.block_number
            ),
        ));
    }
    validate_checkpoint_signing_state(&state, &checkpoint, &options.validator, &member.public_key)
        .map_err(|error| {
            refuse(
                ExternalLaneCheckpointRefusal::SigningStateInvalid,
                format!("`{}`: {error}", state_path.display()),
            )
        })?;
    if let Some(vote) = state.vote {
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
    vote.signature = verified_vote_signature(&private_key, &member.public_key, &signing_bytes)?;
    state.vote = Some(vote.clone());
    write_private_json(&state_path, &state)?;
    write_json(&options.vote_file, &vote)?;
    Ok(vote)
}

/// A vote signature by `private_key`, verified under the member's committee
/// `public_key` before the caller writes the signing state or the vote: a
/// signature that cannot verify would leave a durable vote that can never
/// assemble.
pub(crate) fn verified_vote_signature(
    private_key: &[u8],
    public_key: &[u8],
    signing_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let signature = ml_dsa_65_sign_with_context(
        private_key,
        signing_bytes,
        ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
    )
    .map_err(invalid_data)?;
    if !ml_dsa_65_verify_with_context(
        public_key,
        signing_bytes,
        &signature,
        ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
    ) {
        return Err(refuse(
            ExternalLaneCheckpointRefusal::SigningKeyMismatch,
            "the new vote does not verify under the member's committee public key",
        ));
    }
    Ok(signature)
}

/// `external-lane-checkpoint-assemble`: the certificate of exactly the
/// committee quorum, from votes of distinct active members on this
/// checkpoint, verified with `verify_ethereum_checkpoint_certificate` before
/// it is written. With more valid votes than the quorum it keeps the quorum
/// lowest validator ids.
pub fn assemble_external_lane_checkpoint_certificate(
    options: ExternalLaneCheckpointAssembleOptions,
) -> io::Result<EthereumCheckpointCertificateV1> {
    let checkpoint: EthereumFinalizedCheckpointV1 =
        read_json_file(&options.checkpoint_file, "external lane checkpoint")?;
    let lane = active_lane(&options.data_dir, &checkpoint.route_id)?;
    lane.require_checkpoint(&checkpoint)?;
    let quorum = usize::from(lane.committee.domain.quorum);
    let mut seen = BTreeSet::new();
    let mut votes = Vec::with_capacity(options.vote_files.len());
    for path in &options.vote_files {
        let vote: EthereumCheckpointVoteV1 = read_json_file(path, "external lane checkpoint vote")?;
        if !seen.insert(vote.validator_id.clone()) {
            return Err(refuse(
                ExternalLaneCheckpointRefusal::DuplicateVote,
                format!("two votes of `{}`", vote.validator_id),
            ));
        }
        let member = lane
            .committee
            .validators
            .iter()
            .find(|validator| validator.validator_id == vote.validator_id)
            .ok_or_else(|| {
                refuse(
                    ExternalLaneCheckpointRefusal::VoteNotMember,
                    format!(
                        "`{}` is not a member of the lane's active committee",
                        vote.validator_id
                    ),
                )
            })?;
        let signs_checkpoint = vote.signing_bytes(&checkpoint).is_ok_and(|bytes| {
            ml_dsa_65_verify_with_context(
                &member.public_key,
                &bytes,
                &vote.signature,
                ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
            )
        });
        if !signs_checkpoint {
            return Err(refuse(
                ExternalLaneCheckpointRefusal::VoteOnOtherCheckpoint,
                format!(
                    "the vote of `{}` does not sign this checkpoint",
                    vote.validator_id
                ),
            ));
        }
        votes.push(vote);
    }
    if votes.len() < quorum {
        return Err(refuse(
            ExternalLaneCheckpointRefusal::BelowQuorum,
            format!("{} votes; the committee quorum is {quorum}", votes.len()),
        ));
    }
    votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    votes.truncate(quorum);
    let certificate = EthereumCheckpointCertificateV1 { checkpoint, votes };
    verify_ethereum_checkpoint_certificate(&lane.committee, &certificate).map_err(|error| {
        refuse(
            ExternalLaneCheckpointRefusal::CertificateInvalid,
            format!("{error:?}"),
        )
    })?;
    write_json(&options.certificate_file, &certificate)?;
    Ok(certificate)
}

/// `external-lane-lock-proof`: the `external_lane_release_v1` operation of a
/// Base Lock transaction under a lane checkpoint certificate, written to
/// `operation_file` as the asset operation `wallet-sign-asset-transaction`
/// takes.
pub fn build_external_lane_lock_proof(
    options: ExternalLaneLockProofOptions,
) -> io::Result<AssetTransactionOperation> {
    let lane = active_lane(&options.data_dir, &options.lane_id)?;
    let certificate: EthereumCheckpointCertificateV1 = read_json_file(
        &options.certificate_file,
        "external lane checkpoint certificate",
    )?;
    let checkpoint = &certificate.checkpoint;
    lane.require_checkpoint(checkpoint)?;
    verify_ethereum_checkpoint_certificate(&lane.committee, &certificate).map_err(|error| {
        refuse(
            ExternalLaneCheckpointRefusal::CertificateInvalid,
            format!("{error:?}"),
        )
    })?;
    let endpoint = EthereumRpcEndpoint::parse(&options.base_rpc)?;
    lane.require_chain_id(&endpoint)?;
    let built = receipt_proof_from_rpc(
        &endpoint,
        lane.lane.policy.chain_id,
        &options.transaction_hash,
    )?;
    if built.block_number != checkpoint.block_number
        || exact_hex::<32>("receipt block hash", &built.block_hash)? != checkpoint.block_hash
        || built.receipts_root != checkpoint.receipts_root
    {
        return Err(refuse(
            ExternalLaneCheckpointRefusal::ReceiptBlockMismatch,
            format!(
                "the Lock transaction is in block {} {}, not the checkpoint's block {}",
                built.block_number, built.block_hash, checkpoint.block_number
            ),
        ));
    }

    let logs = receipt_logs(&built.receipts_root, &built.proof)?;
    let locked_topic = locked_event_topic();
    let is_locked = |log: &EthereumLogV1| {
        log.emitter == lane.lock_contract && log.topics.first() == Some(&locked_topic)
    };
    let log_index = match options.log_index {
        Some(index) => {
            let log = usize::try_from(index)
                .ok()
                .and_then(|index| logs.get(index))
                .ok_or_else(|| {
                    refuse(
                        ExternalLaneCheckpointRefusal::LockedLogMissing,
                        format!("the receipt has no log {index}"),
                    )
                })?;
            if log.emitter != lane.lock_contract {
                return Err(refuse(
                    ExternalLaneCheckpointRefusal::LogOfAnotherContract,
                    format!("log {index} is not emitted by the lane's lock contract"),
                ));
            }
            if !is_locked(log) {
                return Err(refuse(
                    ExternalLaneCheckpointRefusal::LogOfAnotherEvent,
                    format!("log {index} is not a `Locked` event"),
                ));
            }
            index
        }
        None => {
            let mut candidates = logs
                .iter()
                .enumerate()
                .filter(|(_, log)| is_locked(log))
                .map(|(index, _)| index);
            let first = candidates.next().ok_or_else(|| {
                refuse(
                    ExternalLaneCheckpointRefusal::LockedLogMissing,
                    "the receipt has no `Locked` event of the lane's lock contract",
                )
            })?;
            if candidates.next().is_some() {
                return Err(refuse(
                    ExternalLaneCheckpointRefusal::LockedLogAmbiguous,
                    "the receipt has several `Locked` events; name one with --log-index",
                ));
            }
            u32::try_from(first).map_err(invalid_data)?
        }
    };
    let log = &logs[usize::try_from(log_index).map_err(invalid_data)?];
    let event = decode_locked_event(log, lane.lock_contract).map_err(|error| {
        refuse(
            ExternalLaneCheckpointRefusal::LockedEventInvalid,
            format!("{error}"),
        )
    })?;
    let recipient = String::from_utf8(event.pftl_recipient.clone())
        .ok()
        .filter(|recipient| ethereum_keccak256(recipient.as_bytes()) == event.pftl_recipient_hash)
        .ok_or_else(|| {
            refuse(
                ExternalLaneCheckpointRefusal::LockedEventInvalid,
                "the Locked recipient is not UTF-8 text matching its indexed hash",
            )
        })?;
    // The token the Lock pulled: an ERC-20 `Transfer` of the amount from the
    // holder to the lock contract, emitted by the lane's token, in the same
    // receipt. Same-shaped transfers of other contracts (a smart-contract
    // wallet, a batch) are ignored: consensus binds the lock's address and
    // code hash, and the lock's token is immutable.
    let transfer_topic = ethereum_keccak256(TRANSFER_EVENT_SIGNATURE);
    let mut amount_word = [0_u8; 32];
    amount_word[24..].copy_from_slice(&event.amount_atoms.to_be_bytes());
    let pulled_by_token = logs.iter().any(|log| {
        log.emitter == lane.token
            && log.topics.len() == 3
            && log.topics[0] == transfer_topic
            && log.topics[1][12..] == event.holder
            && log.topics[1][..12] == [0; 12]
            && log.topics[2][12..] == lane.lock_contract
            && log.topics[2][..12] == [0; 12]
            && log.data == amount_word
    });
    if !pulled_by_token {
        return Err(refuse(
            ExternalLaneCheckpointRefusal::TokenMismatch,
            "the Lock receipt holds no transfer of the amount in the lane's token from the holder to the lock contract",
        ));
    }

    let operation = ExternalLaneReleaseOperationV1 {
        submitter: options.submitter,
        lane_id: lane.lane.id.clone(),
        source_chain_id: lane.lane.policy.chain_id,
        lock_contract_address: lane.lane.policy.lock_contract_address.clone(),
        lock_nonce: event.nonce,
        amount_atoms: event.amount_atoms,
        recipient,
        proof: EthereumExternalEventProofV1 {
            checkpoint_certificate: certificate,
            receipt_proof: built.proof,
            log_index,
        },
    };
    operation
        .validate()
        .map_err(|error| refuse(ExternalLaneCheckpointRefusal::OperationInvalid, error))?;
    let operation = AssetTransactionOperation::ExternalLaneReleaseV1(operation);
    write_json(&options.operation_file, &operation)?;
    Ok(operation)
}

/// Every log of the proven receipt, in order, read through the verifier.
fn receipt_logs(
    receipts_root: &[u8; 32],
    proof: &postfiat_types::EthereumReceiptProofV1,
) -> io::Result<Vec<EthereumLogV1>> {
    let mut logs = Vec::new();
    loop {
        match verify_ethereum_receipt_log(*receipts_root, proof, logs.len()) {
            Ok(log) => logs.push(log),
            Err(error) if error.code() == RECEIPT_LOG_INDEX_INVALID => return Ok(logs),
            Err(error) => {
                return Err(refuse(
                    ExternalLaneCheckpointRefusal::LockedEventInvalid,
                    format!("the Lock receipt does not verify: {error}"),
                ))
            }
        }
    }
}
