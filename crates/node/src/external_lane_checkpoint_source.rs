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

use std::io;
use std::path::{Path, PathBuf};

use postfiat_bridge::ethereum_keccak256;
use postfiat_storage::NodeStore;
use postfiat_types::{
    EthereumFinalizedCheckpointV1, FastSwapChainDomainV1, FastSwapCommitteeV1,
    FastSwapOpaqueHashV1, GenesisExternalLane, ETHEREUM_CHECKPOINT_SCHEMA_V1,
};
use serde_json::Value;

use crate::ethereum_checkpoint_signing::{
    ethereum_rpc_call, ethereum_rpc_hex_bytes, exact_evm_address, exact_hex, invalid_data,
    rpc_hex_u64, value_hex_array, value_hex_u64, write_json, EthereumRpcEndpoint,
};

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
