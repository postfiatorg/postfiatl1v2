// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "./MarketOpsEnvelope.sol";
import {PolicyRegistry} from "./PolicyRegistry.sol";

/// @notice Controlled-launch adapter for PFTL-finalized NAVCoin market-operation envelopes.
/// @dev This is an optimistic controlled-mode bridge: approved proposers post PFTL output,
///      permissionless challenges freeze disputed envelopes, and equivocation pauses the adapter.
contract PFTLBridgeAdapter {
    enum EnvelopeStatus {
        None,
        Pending,
        Challenged,
        Accepted,
        Frozen
    }

    enum ChallengeFault {
        WrongPolicyHash,
        WrongBindings,
        StaleDataWindow,
        TimingViolation,
        HashMismatch,
        Equivocation
    }

    struct PendingEnvelope {
        MarketOpsEnvelope envelope;
        bytes envelope_hash;
        bytes32 envelope_hash_commitment;
        bytes32 evm_envelope_digest;
        address proposer;
        address challenger;
        uint64 posted_at;
        ChallengeFault challenge_fault;
        EnvelopeStatus status;
    }

    error NotOwner();
    error ZeroOwner();
    error ZeroAddress(bytes32 field);
    error NotApprovedProposer(address proposer);
    error AdapterPaused();
    error InvalidEnvelopeHashLength(uint256 length);
    error WrongBinding(bytes32 field);
    error DataWindowInvalid();
    error DataWindowStale(uint64 posted_at, uint64 data_window_end, uint64 max_staleness);
    error TimingViolation(bytes32 rule);
    error PolicyNotAccepted();
    error StaleEpoch(bytes32 asset_id, uint64 epoch, uint64 latest_epoch);
    error DuplicateEnvelope(bytes32 pending_id);
    error UnknownEnvelope(bytes32 pending_id);
    error InvalidEnvelopeStatus(bytes32 pending_id, EnvelopeStatus status);
    error ChallengeDelayOpen(uint64 now_timestamp, uint64 finalizable_at);
    error EnvelopeExpired(uint64 now_timestamp, uint64 expires_at);
    error TimestampOverflow(uint256 timestamp);

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event ProposerApprovalSet(address indexed proposer, bool approved);
    event EnvelopeSubmitted(
        bytes32 indexed pending_id,
        bytes32 indexed asset_id,
        uint64 indexed epoch,
        bytes32 envelope_hash_commitment,
        address proposer,
        uint64 posted_at
    );
    event EnvelopeChallenged(bytes32 indexed pending_id, ChallengeFault fault, address indexed challenger);
    event EnvelopeAccepted(bytes32 indexed pending_id, bytes32 indexed asset_id, uint64 indexed epoch);
    event EnvelopeFrozen(bytes32 indexed pending_id, ChallengeFault fault);
    event AdapterPausedForEquivocation(
        bytes32 indexed asset_epoch_key, bytes32 existing_hash_commitment, bytes32 attempted_hash_commitment
    );
    event AdapterPausedSet(bool paused);

    uint256 public constant ENVELOPE_HASH_BYTES = 48;
    uint32 public constant ENCODING_VERSION = 1;

    PolicyRegistry public immutable policy_registry;
    uint64 public immutable expected_chain_id;
    address public immutable vault_address;
    address public immutable mint_controller_address;
    uint64 public immutable challenge_delay;
    uint64 public immutable execution_window;
    uint64 public immutable max_staleness;

    address public owner;
    bool public paused;

    mapping(address => bool) public approved_proposer;
    mapping(bytes32 => PendingEnvelope) private envelopes;
    mapping(bytes32 => bytes32) public envelope_hash_commitment_by_asset_epoch;
    mapping(bytes32 => uint64) public latest_epoch_by_asset;
    mapping(bytes32 => bytes32) public accepted_envelope_by_asset_epoch;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier onlyApprovedProposer() {
        if (!approved_proposer[msg.sender]) {
            revert NotApprovedProposer(msg.sender);
        }
        _;
    }

    constructor(
        PolicyRegistry registry,
        address initial_owner,
        uint64 chain_id,
        address vault,
        address mint_controller,
        uint64 challenge_delay_seconds,
        uint64 execution_window_seconds,
        uint64 max_staleness_seconds
    ) {
        if (address(registry) == address(0)) {
            revert ZeroAddress("policy_registry");
        }
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        if (vault == address(0)) {
            revert ZeroAddress("vault_address");
        }
        if (mint_controller == address(0)) {
            revert ZeroAddress("mint_controller_address");
        }
        if (chain_id == 0) {
            revert WrongBinding("chain_id");
        }
        if (challenge_delay_seconds == 0) {
            revert TimingViolation("challenge_delay");
        }
        if (execution_window_seconds == 0) {
            revert TimingViolation("execution_window");
        }

        policy_registry = registry;
        owner = initial_owner;
        expected_chain_id = chain_id;
        vault_address = vault;
        mint_controller_address = mint_controller;
        challenge_delay = challenge_delay_seconds;
        execution_window = execution_window_seconds;
        max_staleness = max_staleness_seconds;
        approved_proposer[initial_owner] = true;

        emit OwnershipTransferred(address(0), initial_owner);
        emit ProposerApprovalSet(initial_owner, true);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setProposerApproval(address proposer, bool approved) external onlyOwner {
        if (proposer == address(0)) {
            revert ZeroAddress("proposer");
        }
        approved_proposer[proposer] = approved;
        emit ProposerApprovalSet(proposer, approved);
    }

    function setPaused(bool paused_) external onlyOwner {
        paused = paused_;
        emit AdapterPausedSet(paused_);
    }

    function submitEnvelope(MarketOpsEnvelope calldata envelope, bytes calldata envelope_hash)
        external
        onlyApprovedProposer
        returns (bytes32 pending_id)
    {
        if (paused) {
            revert AdapterPaused();
        }
        if (envelope_hash.length != ENVELOPE_HASH_BYTES) {
            revert InvalidEnvelopeHashLength(envelope_hash.length);
        }

        bytes32 hash_commitment = keccak256(envelope_hash);
        bytes32 asset_epoch_key = assetEpochKey(envelope.asset_id, envelope.epoch);
        bytes32 existing_hash_commitment = envelope_hash_commitment_by_asset_epoch[asset_epoch_key];
        if (existing_hash_commitment != bytes32(0)) {
            if (existing_hash_commitment != hash_commitment) {
                paused = true;
                emit AdapterPausedForEquivocation(asset_epoch_key, existing_hash_commitment, hash_commitment);
                emit AdapterPausedSet(true);
                return bytes32(0);
            }
            revert DuplicateEnvelope(pendingIdFor(envelope.asset_id, envelope.epoch, hash_commitment));
        }

        uint64 latest_epoch = latest_epoch_by_asset[envelope.asset_id];
        if (envelope.epoch < latest_epoch) {
            revert StaleEpoch(envelope.asset_id, envelope.epoch, latest_epoch);
        }

        uint64 posted_at = _now64();
        _validateEnvelopeForSubmission(envelope, posted_at);

        pending_id = pendingIdFor(envelope.asset_id, envelope.epoch, hash_commitment);
        PendingEnvelope storage record = envelopes[pending_id];
        if (record.status != EnvelopeStatus.None) {
            revert DuplicateEnvelope(pending_id);
        }

        record.envelope = envelope;
        record.envelope_hash = envelope_hash;
        record.envelope_hash_commitment = hash_commitment;
        record.evm_envelope_digest = evmEnvelopeDigest(envelope);
        record.proposer = msg.sender;
        record.posted_at = posted_at;
        record.status = EnvelopeStatus.Pending;

        envelope_hash_commitment_by_asset_epoch[asset_epoch_key] = hash_commitment;
        if (envelope.epoch > latest_epoch) {
            latest_epoch_by_asset[envelope.asset_id] = envelope.epoch;
        }

        emit EnvelopeSubmitted(pending_id, envelope.asset_id, envelope.epoch, hash_commitment, msg.sender, posted_at);
    }

    function challengeEnvelope(bytes32 pending_id, ChallengeFault fault) external {
        PendingEnvelope storage record = envelopes[pending_id];
        if (record.status == EnvelopeStatus.None) {
            revert UnknownEnvelope(pending_id);
        }
        if (record.status != EnvelopeStatus.Pending) {
            revert InvalidEnvelopeStatus(pending_id, record.status);
        }

        record.status = EnvelopeStatus.Challenged;
        record.challenger = msg.sender;
        record.challenge_fault = fault;

        if (fault == ChallengeFault.Equivocation) {
            paused = true;
            emit AdapterPausedSet(true);
        }

        emit EnvelopeChallenged(pending_id, fault, msg.sender);
    }

    function finalizeEnvelope(bytes32 pending_id) external {
        PendingEnvelope storage record = envelopes[pending_id];
        if (record.status == EnvelopeStatus.None) {
            revert UnknownEnvelope(pending_id);
        }
        if (record.status != EnvelopeStatus.Pending && record.status != EnvelopeStatus.Challenged) {
            revert InvalidEnvelopeStatus(pending_id, record.status);
        }

        uint64 now_timestamp = _now64();
        uint64 finalizable_at = _checkedAdd64(record.posted_at, challenge_delay, "challenge_delay");
        if (now_timestamp < finalizable_at) {
            revert ChallengeDelayOpen(now_timestamp, finalizable_at);
        }

        if (record.status == EnvelopeStatus.Challenged) {
            record.status = EnvelopeStatus.Frozen;
            emit EnvelopeFrozen(pending_id, record.challenge_fault);
            return;
        }

        if (now_timestamp > record.envelope.expires_at) {
            record.status = EnvelopeStatus.Frozen;
            emit EnvelopeFrozen(pending_id, ChallengeFault.TimingViolation);
            return;
        }
        if (!policy_registry.isEnvelopeAccepted(record.envelope)) {
            record.status = EnvelopeStatus.Frozen;
            emit EnvelopeFrozen(pending_id, ChallengeFault.WrongPolicyHash);
            return;
        }

        record.status = EnvelopeStatus.Accepted;
        accepted_envelope_by_asset_epoch[assetEpochKey(record.envelope.asset_id, record.envelope.epoch)] = pending_id;
        emit EnvelopeAccepted(pending_id, record.envelope.asset_id, record.envelope.epoch);
    }

    function getEnvelopeStatus(bytes32 pending_id) external view returns (EnvelopeStatus) {
        return envelopes[pending_id].status;
    }

    function getEnvelopeHash(bytes32 pending_id) external view returns (bytes memory) {
        return envelopes[pending_id].envelope_hash;
    }

    function getEnvelopeHashCommitment(bytes32 pending_id) external view returns (bytes32) {
        return envelopes[pending_id].envelope_hash_commitment;
    }

    function getEvmEnvelopeDigest(bytes32 pending_id) external view returns (bytes32) {
        return envelopes[pending_id].evm_envelope_digest;
    }

    function getPostedAt(bytes32 pending_id) external view returns (uint64) {
        return envelopes[pending_id].posted_at;
    }

    function getChallengeFault(bytes32 pending_id) external view returns (ChallengeFault) {
        return envelopes[pending_id].challenge_fault;
    }

    function isEnvelopeExecutable(bytes32 pending_id) public view returns (bool) {
        PendingEnvelope storage record = envelopes[pending_id];
        if (paused || record.status != EnvelopeStatus.Accepted) {
            return false;
        }
        uint256 now_timestamp = block.timestamp;
        return now_timestamp >= record.envelope.valid_after && now_timestamp <= record.envelope.expires_at;
    }

    function reserveDeployCapUsdE8(bytes32 pending_id) external view returns (uint256) {
        if (!isEnvelopeExecutable(pending_id)) {
            return 0;
        }
        return envelopes[pending_id].envelope.max_reserve_deploy_usd_e8;
    }

    function mintCapAtoms(bytes32 pending_id) external view returns (uint256) {
        if (!isEnvelopeExecutable(pending_id)) {
            return 0;
        }
        return envelopes[pending_id].envelope.max_mint_atoms;
    }

    function pendingIdFor(bytes32 asset_id, uint64 epoch, bytes32 envelope_hash_commitment)
        public
        pure
        returns (bytes32)
    {
        return keccak256(abi.encode(asset_id, epoch, envelope_hash_commitment));
    }

    function assetEpochKey(bytes32 asset_id, uint64 epoch) public pure returns (bytes32) {
        return keccak256(abi.encode(asset_id, epoch));
    }

    function evmEnvelopeDigest(MarketOpsEnvelope memory envelope) public pure returns (bytes32) {
        return keccak256(abi.encode(envelope));
    }

    function _validateEnvelopeForSubmission(MarketOpsEnvelope calldata envelope, uint64 posted_at) private view {
        if (envelope.encoding_version != ENCODING_VERSION) {
            revert WrongBinding("encoding_version");
        }
        if (envelope.chain_id != expected_chain_id) {
            revert WrongBinding("chain_id");
        }
        if (envelope.adapter_address != address(this)) {
            revert WrongBinding("adapter_address");
        }
        if (envelope.vault_address != vault_address) {
            revert WrongBinding("vault_address");
        }
        if (envelope.mint_controller_address != mint_controller_address) {
            revert WrongBinding("mint_controller_address");
        }
        if (envelope.data_window_start >= envelope.data_window_end) {
            revert DataWindowInvalid();
        }
        uint64 stale_after = _checkedAdd64(envelope.data_window_end, max_staleness, "max_staleness");
        if (posted_at > stale_after) {
            revert DataWindowStale(posted_at, envelope.data_window_end, max_staleness);
        }
        uint64 valid_after_min = _checkedAdd64(posted_at, challenge_delay, "challenge_delay");
        if (envelope.valid_after < valid_after_min) {
            revert TimingViolation("valid_after");
        }
        if (envelope.expires_at < envelope.valid_after) {
            revert TimingViolation("valid_window");
        }
        uint64 expires_at_max = _checkedAdd64(envelope.valid_after, execution_window, "execution_window");
        if (envelope.expires_at > expires_at_max) {
            revert TimingViolation("expires_at");
        }
        if (posted_at > envelope.expires_at) {
            revert EnvelopeExpired(posted_at, envelope.expires_at);
        }
        if (!policy_registry.isEnvelopeAccepted(envelope)) {
            revert PolicyNotAccepted();
        }
    }

    function _now64() private view returns (uint64) {
        uint256 timestamp = block.timestamp;
        if (timestamp > type(uint64).max) {
            revert TimestampOverflow(timestamp);
        }
        // casting to uint64 is safe because the guard above rejects larger timestamps.
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint64(timestamp);
    }

    function _checkedAdd64(uint64 left, uint64 right, bytes32 rule) private pure returns (uint64) {
        if (right > type(uint64).max - left) {
            revert TimingViolation(rule);
        }
        return left + right;
    }
}
