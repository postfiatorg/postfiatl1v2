// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @notice Controlled-launch verifier for PFTL-finalized vault bridge asset withdrawal packets.
/// @dev Anyone may relay a threshold-signed PFTL withdrawal proof. Anyone may
///      challenge before the delay closes. Challenged packets freeze and never
///      authorize vault payment.
contract PFTLWithdrawalVerifier {
    enum ProofStatus {
        None,
        Pending,
        Challenged,
        Accepted,
        Frozen
    }

    enum ChallengeFault {
        WrongPFTLHash,
        WrongPacketDigest,
        WrongFinalityHeight,
        BadSignatureSet,
        Replay,
        Other
    }

    struct PendingProof {
        bytes32 packet_digest;
        bytes32 pftl_withdrawal_hash_commitment;
        uint64 pftl_finalized_height;
        address proposer;
        address challenger;
        uint64 posted_at;
        uint64 valid_after;
        uint64 expires_at;
        ChallengeFault challenge_fault;
        ProofStatus status;
    }

    error NotOwner();
    error NotChallengeAuthority();
    error ZeroOwner();
    error ZeroAddress(bytes32 field);
    error InvalidThreshold(uint256 threshold, uint256 signer_count);
    error InvalidSignatureCount(uint256 count, uint256 threshold);
    error BadSignatureLength(uint256 length);
    error InvalidSigner(address signer);
    error DuplicateOrUnsortedSigner(address signer);
    error BadSignature();
    error ZeroDigest(bytes32 field);
    error ZeroHeight();
    error TimingViolation(bytes32 rule);
    error DuplicateProof(bytes32 pending_id);
    error UnknownProof(bytes32 pending_id);
    error InvalidProofStatus(bytes32 pending_id, ProofStatus status);
    error ChallengeWindowOpen(uint64 now_timestamp, uint64 valid_after);
    error TimestampOverflow(uint256 timestamp);

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event ChallengeAuthoritySet(address indexed previous_authority, address indexed new_authority);
    event SignerSet(address indexed signer, bool approved);
    event ThresholdSet(uint256 threshold);
    event ProofSubmitted(
        bytes32 indexed pending_id,
        bytes32 indexed packet_digest,
        bytes32 indexed pftl_withdrawal_hash_commitment,
        uint64 pftl_finalized_height,
        address proposer,
        uint64 posted_at,
        uint64 valid_after,
        uint64 expires_at
    );
    event ProofChallenged(bytes32 indexed pending_id, ChallengeFault fault, address indexed challenger);
    event ProofAccepted(bytes32 indexed pending_id, bytes32 indexed packet_digest);
    event ProofFrozen(bytes32 indexed pending_id, ChallengeFault fault);

    uint256 private constant SECP256K1N_HALF = 0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;

    address public owner;
    address public challenge_authority;
    uint64 public immutable challenge_delay;
    uint64 public immutable execution_window;
    uint256 public signer_count;
    uint256 public threshold;

    mapping(address => bool) public is_signer;
    mapping(bytes32 => PendingProof) private proofs;
    mapping(bytes32 => bytes32) public accepted_proof_by_withdrawal_key;

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

    constructor(
        address initial_owner,
        address[] memory initial_signers,
        uint256 threshold_,
        uint64 challenge_delay_seconds,
        uint64 execution_window_seconds
    ) {
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        if (challenge_delay_seconds == 0) {
            revert TimingViolation("challenge_delay");
        }
        if (execution_window_seconds == 0) {
            revert TimingViolation("execution_window");
        }
        owner = initial_owner;
        challenge_authority = initial_owner;
        challenge_delay = challenge_delay_seconds;
        execution_window = execution_window_seconds;
        emit OwnershipTransferred(address(0), initial_owner);
        emit ChallengeAuthoritySet(address(0), initial_owner);

        for (uint256 i = 0; i < initial_signers.length; i++) {
            _setSigner(initial_signers[i], true);
        }
        _setThreshold(threshold_);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setSigner(address signer, bool approved) external onlyOwner {
        _setSigner(signer, approved);
        if (threshold == 0 || threshold > signer_count) {
            revert InvalidThreshold(threshold, signer_count);
        }
    }

    function setThreshold(uint256 threshold_) external onlyOwner {
        _setThreshold(threshold_);
    }

    function setChallengeAuthority(address new_authority) external onlyOwner {
        if (new_authority == address(0)) {
            revert ZeroAddress("challenge_authority");
        }
        emit ChallengeAuthoritySet(challenge_authority, new_authority);
        challenge_authority = new_authority;
    }

    function submitProof(
        bytes32 packet_digest,
        bytes32 pftl_withdrawal_hash_commitment,
        uint64 pftl_finalized_height,
        bytes[] calldata signatures
    ) external returns (bytes32 pending_id) {
        _validateProofFields(packet_digest, pftl_withdrawal_hash_commitment, pftl_finalized_height);
        _validateSignatures(
            proofDigest(packet_digest, pftl_withdrawal_hash_commitment, pftl_finalized_height), signatures
        );

        pending_id = pendingProofId(packet_digest, pftl_withdrawal_hash_commitment, pftl_finalized_height);
        PendingProof storage record = proofs[pending_id];
        if (record.status != ProofStatus.None) {
            revert DuplicateProof(pending_id);
        }

        uint64 posted_at = _now64();
        record.packet_digest = packet_digest;
        record.pftl_withdrawal_hash_commitment = pftl_withdrawal_hash_commitment;
        record.pftl_finalized_height = pftl_finalized_height;
        record.proposer = msg.sender;
        record.posted_at = posted_at;
        record.valid_after = _checkedAdd64(posted_at, challenge_delay, "challenge_delay");
        record.expires_at = _checkedAdd64(record.valid_after, execution_window, "execution_window");
        record.status = ProofStatus.Pending;

        emit ProofSubmitted(
            pending_id,
            packet_digest,
            pftl_withdrawal_hash_commitment,
            pftl_finalized_height,
            msg.sender,
            record.posted_at,
            record.valid_after,
            record.expires_at
        );
    }

    function challengeProof(bytes32 pending_id, ChallengeFault fault) external onlyChallengeAuthority {
        PendingProof storage record = proofs[pending_id];
        if (record.status == ProofStatus.None) {
            revert UnknownProof(pending_id);
        }
        if (record.status != ProofStatus.Pending) {
            revert InvalidProofStatus(pending_id, record.status);
        }
        record.status = ProofStatus.Challenged;
        record.challenger = msg.sender;
        record.challenge_fault = fault;
        emit ProofChallenged(pending_id, fault, msg.sender);
    }

    function finalizeProof(bytes32 pending_id) external {
        PendingProof storage record = proofs[pending_id];
        if (record.status == ProofStatus.None) {
            revert UnknownProof(pending_id);
        }
        if (record.status != ProofStatus.Pending && record.status != ProofStatus.Challenged) {
            revert InvalidProofStatus(pending_id, record.status);
        }
        uint64 now_timestamp = _now64();
        if (now_timestamp < record.valid_after) {
            revert ChallengeWindowOpen(now_timestamp, record.valid_after);
        }
        if (record.status == ProofStatus.Challenged) {
            record.status = ProofStatus.Frozen;
            emit ProofFrozen(pending_id, record.challenge_fault);
            return;
        }
        record.status = ProofStatus.Accepted;
        accepted_proof_by_withdrawal_key[withdrawalKey(record.packet_digest, record.pftl_withdrawal_hash_commitment)] =
            pending_id;
        emit ProofAccepted(pending_id, record.packet_digest);
    }

    function isWithdrawalAccepted(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment)
        external
        view
        returns (bool)
    {
        bytes32 pending_id =
            accepted_proof_by_withdrawal_key[withdrawalKey(packet_digest, pftl_withdrawal_hash_commitment)];
        if (pending_id == bytes32(0)) {
            return false;
        }
        PendingProof storage record = proofs[pending_id];
        return record.status == ProofStatus.Accepted && block.timestamp <= record.expires_at;
    }

    function getProofStatus(bytes32 pending_id) external view returns (ProofStatus) {
        return proofs[pending_id].status;
    }

    function getProofExpiresAt(bytes32 pending_id) external view returns (uint64) {
        return proofs[pending_id].expires_at;
    }

    function pendingProofId(
        bytes32 packet_digest,
        bytes32 pftl_withdrawal_hash_commitment,
        uint64 pftl_finalized_height
    ) public pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.erc20_bridge.withdrawal_proof_pending.v1",
                packet_digest,
                pftl_withdrawal_hash_commitment,
                pftl_finalized_height
            )
        );
    }

    function withdrawalKey(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment)
        public
        pure
        returns (bytes32)
    {
        return keccak256(
            abi.encode("postfiat.erc20_bridge.withdrawal_key.v1", packet_digest, pftl_withdrawal_hash_commitment)
        );
    }

    function proofDigest(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment, uint64 pftl_finalized_height)
        public
        view
        returns (bytes32)
    {
        _validateProofFields(packet_digest, pftl_withdrawal_hash_commitment, pftl_finalized_height);
        return keccak256(
            abi.encode(
                "postfiat.erc20_bridge.withdrawal_proof.v1",
                block.chainid,
                address(this),
                packet_digest,
                pftl_withdrawal_hash_commitment,
                pftl_finalized_height
            )
        );
    }

    function _setSigner(address signer, bool approved) private {
        if (signer == address(0)) {
            revert ZeroAddress("signer");
        }
        bool current = is_signer[signer];
        if (current == approved) {
            return;
        }
        is_signer[signer] = approved;
        if (approved) {
            signer_count += 1;
        } else {
            signer_count -= 1;
        }
        emit SignerSet(signer, approved);
    }

    function _setThreshold(uint256 threshold_) private {
        if (threshold_ == 0 || threshold_ > signer_count) {
            revert InvalidThreshold(threshold_, signer_count);
        }
        threshold = threshold_;
        emit ThresholdSet(threshold_);
    }

    function _validateProofFields(
        bytes32 packet_digest,
        bytes32 pftl_withdrawal_hash_commitment,
        uint64 pftl_finalized_height
    ) private pure {
        if (packet_digest == bytes32(0)) {
            revert ZeroDigest("packet_digest");
        }
        if (pftl_withdrawal_hash_commitment == bytes32(0)) {
            revert ZeroDigest("pftl_withdrawal_hash_commitment");
        }
        if (pftl_finalized_height == 0) {
            revert ZeroHeight();
        }
    }

    function _validateSignatures(bytes32 digest, bytes[] calldata signatures) private view {
        if (signatures.length < threshold) {
            revert InvalidSignatureCount(signatures.length, threshold);
        }
        address previous = address(0);
        for (uint256 i = 0; i < signatures.length; i++) {
            address signer = _recover(digest, signatures[i]);
            if (!is_signer[signer]) {
                revert InvalidSigner(signer);
            }
            if (signer <= previous) {
                revert DuplicateOrUnsortedSigner(signer);
            }
            previous = signer;
        }
    }

    function _recover(bytes32 digest, bytes calldata signature) private pure returns (address signer) {
        if (signature.length != 65) {
            revert BadSignatureLength(signature.length);
        }
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := calldataload(signature.offset)
            s := calldataload(add(signature.offset, 32))
            v := byte(0, calldataload(add(signature.offset, 64)))
        }
        if (v != 27 && v != 28) {
            revert BadSignature();
        }
        if (uint256(s) > SECP256K1N_HALF) {
            revert BadSignature();
        }
        signer = ecrecover(digest, v, r, s);
        if (signer == address(0)) {
            revert BadSignature();
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
