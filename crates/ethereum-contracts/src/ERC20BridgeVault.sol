// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IERC20BridgeToken {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

interface IPFTLWithdrawalVerifier {
    function isWithdrawalAccepted(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment)
        external
        view
        returns (bool);
}

/// @notice Source-chain ERC20 vault for vault bridge asset deposits and PFTL-finalized withdrawals.
/// @dev Deposits are canonical EVM events for PFTL replay. Withdrawals require
///      an accepted PFTL withdrawal verifier record before the vault queues a
///      claim; the vault then applies its own challenge/finality window and
///      pays ERC20 directly to the destination recipient.
contract ERC20BridgeVault {
    enum WithdrawalStatus {
        None,
        Pending,
        Challenged,
        Accepted,
        Claimed,
        Frozen
    }

    enum ChallengeFault {
        WrongPFTLChain,
        WrongAsset,
        WrongRecipient,
        WrongAmount,
        TimingViolation,
        HashMismatch,
        Replay,
        Other
    }

    struct WithdrawalPacket {
        uint64 pftl_chain_id;
        uint256 source_chain_id;
        address vault_address;
        address token_address;
        bytes vault_bridge_asset_id;
        bytes burn_tx_id;
        bytes withdrawal_id;
        address recipient;
        uint256 amount;
        bytes source_bucket_id;
        bytes destination_hash;
        uint64 finalized_height;
        bytes evidence_root;
    }

    struct PendingWithdrawal {
        WithdrawalPacket packet;
        bytes pftl_withdrawal_hash;
        bytes32 pftl_withdrawal_hash_commitment;
        bytes32 packet_digest;
        address proposer;
        address challenger;
        uint64 posted_at;
        uint64 valid_after;
        uint64 expires_at;
        ChallengeFault challenge_fault;
        WithdrawalStatus status;
    }

    error NotOwner();
    error NotChallengeAuthority();
    error ZeroOwner();
    error ZeroAddress(bytes32 field);
    error VaultPaused();
    error InvalidAmount();
    error RecipientTextEmpty();
    error RecipientTextTooLong(uint256 length);
    error RouteBindingRequired();
    error DuplicateDeposit(bytes32 deposit_id);
    error InvalidPFTLHashLength(uint256 length);
    error WrongPFTLChain(uint64 actual, uint64 expected);
    error WrongSourceChain(uint256 actual, uint256 expected);
    error WrongVault(address actual, address expected);
    error WrongToken(address actual, address expected);
    error WrongAsset(bytes actual, bytes expected);
    error BadWithdrawalPacket(bytes32 field);
    error TimingViolation(bytes32 rule);
    error DuplicateWithdrawal(bytes32 pending_id);
    error BurnAlreadySubmitted(bytes32 burn_tx_id_commitment);
    error UnknownWithdrawal(bytes32 pending_id);
    error InvalidWithdrawalStatus(bytes32 pending_id, WithdrawalStatus status);
    error ChallengeWindowOpen(uint64 now_timestamp, uint64 valid_after);
    error WithdrawalNotVerified(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment);
    error InsufficientVaultLiquidity(uint256 requested, uint256 available);
    error TokenTransferFailed();
    error TokenTransferFromFailed();
    error TimestampOverflow(uint256 timestamp);

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event ChallengeAuthoritySet(address indexed previous_authority, address indexed new_authority);
    event PausedSet(bool paused);
    event ERC20BridgeDeposited(
        bytes32 indexed deposit_id,
        address indexed depositor,
        bytes32 indexed pftl_recipient_hash,
        string pftl_recipient,
        uint256 amount,
        bytes32 nonce,
        uint256 source_chain_id,
        address vault,
        address token
    );
    event ERC20BridgeDepositedV2(
        bytes32 indexed deposit_id,
        address indexed depositor,
        bytes32 indexed pftl_recipient_hash,
        string pftl_recipient,
        uint256 amount,
        bytes32 nonce,
        bytes32 route_binding,
        uint256 source_chain_id,
        address vault,
        address token
    );
    event WithdrawalSubmitted(
        bytes32 indexed pending_id,
        bytes32 indexed withdrawal_id_commitment,
        bytes32 indexed burn_tx_id_commitment,
        bytes32 pftl_withdrawal_hash_commitment,
        address proposer,
        address recipient,
        uint256 amount,
        uint64 posted_at,
        uint64 valid_after,
        uint64 expires_at
    );
    event WithdrawalChallenged(bytes32 indexed pending_id, ChallengeFault fault, address indexed challenger);
    event WithdrawalAccepted(bytes32 indexed pending_id, bytes32 indexed withdrawal_id_commitment);
    event WithdrawalFrozen(bytes32 indexed pending_id, ChallengeFault fault);
    event WithdrawalClaimed(
        bytes32 indexed pending_id, bytes32 indexed withdrawal_id_commitment, address indexed recipient, uint256 amount
    );

    uint256 public constant PFTL_WITHDRAWAL_HASH_BYTES = 48;
    uint256 public constant PFTL_FIELD_HASH_BYTES = 48;
    uint256 public constant MAX_PFTL_RECIPIENT_BYTES = 256;

    IERC20BridgeToken public immutable token;
    IPFTLWithdrawalVerifier public immutable withdrawal_verifier;
    uint64 public immutable expected_pftl_chain_id;
    bytes public vault_bridge_asset_id;
    uint64 public immutable challenge_delay;
    uint64 public immutable execution_window;

    address public owner;
    address public challenge_authority;
    bool public paused;

    mapping(bytes32 => bool) public deposit_seen;
    mapping(bytes32 => PendingWithdrawal) private withdrawals;
    mapping(bytes32 => bytes32) public pending_id_by_burn_tx;
    mapping(bytes32 => bool) public claimed_withdrawal_id;

    uint256 private reentrancy_lock;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier onlyChallengeAuthority() {
        if (msg.sender != challenge_authority && msg.sender != owner) {
            revert NotChallengeAuthority();
        }
        _;
    }

    modifier nonReentrant() {
        if (reentrancy_lock != 0) {
            revert("reentrant");
        }
        reentrancy_lock = 1;
        _;
        reentrancy_lock = 0;
    }

    constructor(
        IERC20BridgeToken token_,
        IPFTLWithdrawalVerifier withdrawal_verifier_,
        address initial_owner,
        uint64 expected_pftl_chain_id_,
        bytes memory vault_bridge_asset_id_,
        uint64 challenge_delay_seconds,
        uint64 execution_window_seconds
    ) {
        if (address(token_) == address(0)) {
            revert ZeroAddress("token");
        }
        if (address(withdrawal_verifier_) == address(0)) {
            revert ZeroAddress("withdrawal_verifier");
        }
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        if (expected_pftl_chain_id_ == 0) {
            revert WrongPFTLChain(0, expected_pftl_chain_id_);
        }
        if (vault_bridge_asset_id_.length != PFTL_FIELD_HASH_BYTES) {
            revert BadWithdrawalPacket("vault_bridge_asset_id");
        }
        if (challenge_delay_seconds == 0) {
            revert TimingViolation("challenge_delay");
        }
        if (execution_window_seconds == 0) {
            revert TimingViolation("execution_window");
        }

        token = token_;
        withdrawal_verifier = withdrawal_verifier_;
        owner = initial_owner;
        challenge_authority = initial_owner;
        expected_pftl_chain_id = expected_pftl_chain_id_;
        vault_bridge_asset_id = vault_bridge_asset_id_;
        challenge_delay = challenge_delay_seconds;
        execution_window = execution_window_seconds;

        emit OwnershipTransferred(address(0), initial_owner);
        emit ChallengeAuthoritySet(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setPaused(bool paused_) external onlyOwner {
        paused = paused_;
        emit PausedSet(paused_);
    }

    function setChallengeAuthority(address new_authority) external onlyOwner {
        if (new_authority == address(0)) {
            revert ZeroAddress("challenge_authority");
        }
        emit ChallengeAuthoritySet(challenge_authority, new_authority);
        challenge_authority = new_authority;
    }

    function deposit(uint256, string calldata, bytes32) external pure returns (bytes32) {
        revert RouteBindingRequired();
    }

    /// @notice Deposit ERC20 while binding the user-signed transaction to the
    /// exact governed PFTL bridge route selected by the wallet.
    function depositV2(uint256 amount, string calldata pftl_recipient, bytes32 nonce, bytes32 route_binding)
        external
        nonReentrant
        returns (bytes32 deposit_id)
    {
        if (paused) {
            revert VaultPaused();
        }
        if (amount == 0) {
            revert InvalidAmount();
        }
        bytes calldata recipient_bytes = bytes(pftl_recipient);
        if (recipient_bytes.length == 0) {
            revert RecipientTextEmpty();
        }
        if (recipient_bytes.length > MAX_PFTL_RECIPIENT_BYTES) {
            revert RecipientTextTooLong(recipient_bytes.length);
        }
        if (route_binding == bytes32(0)) {
            revert RouteBindingRequired();
        }

        bytes32 recipient_hash = keccak256(recipient_bytes);
        deposit_id = depositIdV2(msg.sender, amount, recipient_hash, nonce, route_binding);
        if (deposit_seen[deposit_id]) {
            revert DuplicateDeposit(deposit_id);
        }
        deposit_seen[deposit_id] = true;

        _safeTransferFrom(msg.sender, address(this), amount);
        emit ERC20BridgeDepositedV2(
            deposit_id,
            msg.sender,
            recipient_hash,
            pftl_recipient,
            amount,
            nonce,
            route_binding,
            block.chainid,
            address(this),
            address(token)
        );
    }

    function submitWithdrawal(WithdrawalPacket calldata packet, bytes calldata pftl_withdrawal_hash)
        external
        returns (bytes32 pending_id)
    {
        if (paused) {
            revert VaultPaused();
        }
        if (pftl_withdrawal_hash.length != PFTL_WITHDRAWAL_HASH_BYTES) {
            revert InvalidPFTLHashLength(pftl_withdrawal_hash.length);
        }
        _validatePacket(packet);

        bytes32 burn_commitment = keccak256(packet.burn_tx_id);
        bytes32 withdrawal_commitment = keccak256(packet.withdrawal_id);
        bytes32 existing = pending_id_by_burn_tx[burn_commitment];
        if (existing != bytes32(0)) {
            revert BurnAlreadySubmitted(burn_commitment);
        }
        if (claimed_withdrawal_id[withdrawal_commitment]) {
            revert BadWithdrawalPacket("withdrawal_id");
        }

        bytes32 hash_commitment = keccak256(pftl_withdrawal_hash);
        bytes32 packet_digest = withdrawalPacketDigest(packet);
        if (!withdrawal_verifier.isWithdrawalAccepted(packet_digest, hash_commitment)) {
            revert WithdrawalNotVerified(packet_digest, hash_commitment);
        }
        pending_id = withdrawalPendingId(packet, hash_commitment);
        PendingWithdrawal storage record = withdrawals[pending_id];
        if (record.status != WithdrawalStatus.None) {
            revert DuplicateWithdrawal(pending_id);
        }

        record.packet = packet;
        record.pftl_withdrawal_hash = pftl_withdrawal_hash;
        record.pftl_withdrawal_hash_commitment = hash_commitment;
        record.packet_digest = packet_digest;
        record.proposer = msg.sender;
        record.posted_at = _now64();
        record.valid_after = _checkedAdd64(record.posted_at, challenge_delay, "challenge_delay");
        record.expires_at = _checkedAdd64(record.valid_after, execution_window, "execution_window");
        record.status = WithdrawalStatus.Pending;
        pending_id_by_burn_tx[burn_commitment] = pending_id;

        _emitWithdrawalSubmitted(pending_id, record);
    }

    function challengeWithdrawal(bytes32 pending_id, ChallengeFault fault) external onlyChallengeAuthority {
        PendingWithdrawal storage record = withdrawals[pending_id];
        if (record.status == WithdrawalStatus.None) {
            revert UnknownWithdrawal(pending_id);
        }
        if (record.status != WithdrawalStatus.Pending) {
            revert InvalidWithdrawalStatus(pending_id, record.status);
        }

        record.status = WithdrawalStatus.Challenged;
        record.challenger = msg.sender;
        record.challenge_fault = fault;
        emit WithdrawalChallenged(pending_id, fault, msg.sender);
    }

    function finalizeWithdrawal(bytes32 pending_id) external {
        PendingWithdrawal storage record = withdrawals[pending_id];
        if (record.status == WithdrawalStatus.None) {
            revert UnknownWithdrawal(pending_id);
        }
        if (record.status != WithdrawalStatus.Pending && record.status != WithdrawalStatus.Challenged) {
            revert InvalidWithdrawalStatus(pending_id, record.status);
        }

        uint64 now_timestamp = _now64();
        if (now_timestamp < record.valid_after) {
            revert ChallengeWindowOpen(now_timestamp, record.valid_after);
        }
        if (record.status == WithdrawalStatus.Challenged) {
            record.status = WithdrawalStatus.Frozen;
            emit WithdrawalFrozen(pending_id, record.challenge_fault);
            return;
        }

        record.status = WithdrawalStatus.Accepted;
        emit WithdrawalAccepted(pending_id, keccak256(record.packet.withdrawal_id));
    }

    function claimWithdrawal(bytes32 pending_id) external nonReentrant {
        PendingWithdrawal storage record = withdrawals[pending_id];
        if (record.status == WithdrawalStatus.None) {
            revert UnknownWithdrawal(pending_id);
        }
        if (record.status != WithdrawalStatus.Accepted) {
            revert InvalidWithdrawalStatus(pending_id, record.status);
        }

        uint256 available = token.balanceOf(address(this));
        if (available < record.packet.amount) {
            revert InsufficientVaultLiquidity(record.packet.amount, available);
        }

        WithdrawalPacket memory packet = record.packet;
        record.status = WithdrawalStatus.Claimed;
        bytes32 withdrawal_commitment = keccak256(packet.withdrawal_id);
        claimed_withdrawal_id[withdrawal_commitment] = true;
        _safeTransfer(packet.recipient, packet.amount);

        emit WithdrawalClaimed(pending_id, withdrawal_commitment, packet.recipient, packet.amount);
    }

    function getWithdrawalStatus(bytes32 pending_id) external view returns (WithdrawalStatus) {
        return withdrawals[pending_id].status;
    }

    function getWithdrawalPacketDigest(bytes32 pending_id) external view returns (bytes32) {
        return withdrawals[pending_id].packet_digest;
    }

    function getWithdrawalHashCommitment(bytes32 pending_id) external view returns (bytes32) {
        return withdrawals[pending_id].pftl_withdrawal_hash_commitment;
    }

    function getWithdrawalAmount(bytes32 pending_id) external view returns (uint256) {
        return withdrawals[pending_id].packet.amount;
    }

    function getWithdrawalRecipient(bytes32 pending_id) external view returns (address) {
        return withdrawals[pending_id].packet.recipient;
    }

    function isWithdrawalClaimable(bytes32 pending_id) external view returns (bool) {
        PendingWithdrawal storage record = withdrawals[pending_id];
        return record.status == WithdrawalStatus.Accepted;
    }

    function depositId(address depositor, uint256 amount, bytes32 pftl_recipient_hash, bytes32 nonce)
        public
        view
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                "postfiat.erc20_bridge.deposit.v1",
                block.chainid,
                address(this),
                address(token),
                depositor,
                amount,
                pftl_recipient_hash,
                nonce
            )
        );
    }

    function depositIdV2(
        address depositor,
        uint256 amount,
        bytes32 pftl_recipient_hash,
        bytes32 nonce,
        bytes32 route_binding
    ) public view returns (bytes32) {
        if (route_binding == bytes32(0)) {
            revert RouteBindingRequired();
        }
        return keccak256(
            abi.encode(
                "postfiat.erc20_bridge.deposit.v2",
                block.chainid,
                address(this),
                address(token),
                depositor,
                amount,
                pftl_recipient_hash,
                nonce,
                route_binding
            )
        );
    }

    function withdrawalPacketDigest(WithdrawalPacket memory packet) public pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.erc20_bridge.withdrawal_packet.v2",
                _withdrawalPacketDomainHash(packet),
                _withdrawalPacketPayloadHash(packet)
            )
        );
    }

    function _withdrawalPacketDomainHash(WithdrawalPacket memory packet) private pure returns (bytes32) {
        return
            keccak256(
                abi.encode(packet.pftl_chain_id, packet.source_chain_id, packet.vault_address, packet.token_address)
            );
    }

    function _withdrawalPacketPayloadHash(WithdrawalPacket memory packet) private pure returns (bytes32) {
        return keccak256(
            abi.encode(
                packet.vault_bridge_asset_id,
                packet.burn_tx_id,
                packet.withdrawal_id,
                packet.recipient,
                packet.amount,
                packet.source_bucket_id,
                packet.destination_hash,
                packet.finalized_height,
                packet.evidence_root
            )
        );
    }

    function withdrawalPendingId(WithdrawalPacket memory packet, bytes32 pftl_withdrawal_hash_commitment)
        public
        pure
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                "postfiat.erc20_bridge.withdrawal_pending.v1",
                packet.withdrawal_id,
                packet.burn_tx_id,
                pftl_withdrawal_hash_commitment,
                withdrawalPacketDigest(packet)
            )
        );
    }

    function _emitWithdrawalSubmitted(bytes32 pending_id, PendingWithdrawal storage record) private {
        WithdrawalPacket storage packet = record.packet;
        emit WithdrawalSubmitted(
            pending_id,
            keccak256(packet.withdrawal_id),
            keccak256(packet.burn_tx_id),
            record.pftl_withdrawal_hash_commitment,
            record.proposer,
            packet.recipient,
            packet.amount,
            record.posted_at,
            record.valid_after,
            record.expires_at
        );
    }

    function _validatePacket(WithdrawalPacket calldata packet) private view {
        if (packet.pftl_chain_id != expected_pftl_chain_id) {
            revert WrongPFTLChain(packet.pftl_chain_id, expected_pftl_chain_id);
        }
        if (packet.source_chain_id != block.chainid) {
            revert WrongSourceChain(packet.source_chain_id, block.chainid);
        }
        if (packet.vault_address != address(this)) {
            revert WrongVault(packet.vault_address, address(this));
        }
        if (packet.token_address != address(token)) {
            revert WrongToken(packet.token_address, address(token));
        }
        if (packet.vault_bridge_asset_id.length != PFTL_FIELD_HASH_BYTES) {
            revert BadWithdrawalPacket("vault_bridge_asset_id");
        }
        if (keccak256(packet.vault_bridge_asset_id) != keccak256(vault_bridge_asset_id)) {
            revert WrongAsset(packet.vault_bridge_asset_id, vault_bridge_asset_id);
        }
        if (!_validPftlHash(packet.burn_tx_id)) {
            revert BadWithdrawalPacket("burn_tx_id");
        }
        if (!_validPftlHash(packet.withdrawal_id)) {
            revert BadWithdrawalPacket("withdrawal_id");
        }
        if (packet.recipient == address(0)) {
            revert ZeroAddress("recipient");
        }
        if (packet.amount == 0) {
            revert InvalidAmount();
        }
        if (!_validPftlHash(packet.source_bucket_id)) {
            revert BadWithdrawalPacket("source_bucket_id");
        }
        if (!_validPftlHash(packet.destination_hash)) {
            revert BadWithdrawalPacket("destination_hash");
        }
        if (packet.finalized_height == 0) {
            revert BadWithdrawalPacket("finalized_height");
        }
        if (!_validPftlHash(packet.evidence_root)) {
            revert BadWithdrawalPacket("evidence_root");
        }
    }

    function _validPftlHash(bytes calldata value) private pure returns (bool) {
        if (value.length != PFTL_FIELD_HASH_BYTES) {
            return false;
        }
        for (uint256 i = 0; i < PFTL_FIELD_HASH_BYTES; i++) {
            if (value[i] != 0) {
                return true;
            }
        }
        return false;
    }

    function _safeTransfer(address to, uint256 amount) private {
        (bool ok, bytes memory data) = address(token).call(abi.encodeCall(IERC20BridgeToken.transfer, (to, amount)));
        if (!ok || (data.length != 0 && !abi.decode(data, (bool)))) {
            revert TokenTransferFailed();
        }
    }

    function _safeTransferFrom(address from, address to, uint256 amount) private {
        (bool ok, bytes memory data) =
            address(token).call(abi.encodeCall(IERC20BridgeToken.transferFrom, (from, to, amount)));
        if (!ok || (data.length != 0 && !abi.decode(data, (bool)))) {
            revert TokenTransferFromFailed();
        }
    }

    function _now64() private view returns (uint64) {
        if (block.timestamp > type(uint64).max) {
            revert TimestampOverflow(block.timestamp);
        }
        return uint64(block.timestamp);
    }

    function _checkedAdd64(uint64 left, uint64 right, bytes32 rule) private pure returns (uint64 result) {
        uint256 sum = uint256(left) + uint256(right);
        if (sum > type(uint64).max) {
            revert TimingViolation(rule);
        }
        result = uint64(sum);
    }
}
