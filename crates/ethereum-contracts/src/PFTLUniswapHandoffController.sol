// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IVenueMintableToken {
    function balanceOf(address account) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function mint(address to, uint256 amount) external;
    function burnFromBridge(address from, uint256 amount) external;
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

interface IERC20Balance {
    function balanceOf(address account) external view returns (uint256);
}

interface IExactInputRouter {
    function exactInput(
        address token_in,
        address token_out,
        uint256 amount_in,
        uint256 minimum_output,
        address recipient,
        uint256 deadline,
        bytes calldata data
    ) external returns (uint256 amount_out);
}

interface IPoolBoundExactInputRouter is IExactInputRouter {
    function uniswap_pool_id() external view returns (bytes32);
}

interface IPFTLReceiptVerifier {
    function routeTrustClass() external view returns (bytes32);
    function isReceiptAccepted(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class,
        bytes32 packet_digest
    ) external view returns (bool);
}

contract ControlledPFTLReceiptVerifier is IPFTLReceiptVerifier {
    error NotOwner();
    error ZeroOwner();
    error BadTrustClass(bytes32 trust_class);
    error InvalidPftlBytes(bytes32 field, uint256 actual_length, uint256 expected_length);
    error ZeroPftlBytes(bytes32 field);
    error ZeroPacketDigest();

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event ReceiptAcceptanceSet(bytes32 indexed receipt_commitment, bool accepted);

    bytes32 public constant TRUST_CLASS_CONTROLLED = keccak256("CONTROLLED");
    bytes32 public constant TRUST_CLASS_OPTIMISTIC = keccak256("OPTIMISTIC");
    bytes32 public constant TRUST_CLASS_TRUSTLESS_FINALITY = keccak256("TRUSTLESS_FINALITY");
    bytes32 public constant TRUST_CLASS_DISABLED = keccak256("DISABLED");

    address public owner;
    bytes32 public immutable route_trust_class;
    mapping(bytes32 => bool) public accepted_receipt;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    constructor(address initial_owner, bytes32 route_trust_class_) {
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        _validateTrustClass(route_trust_class_);
        owner = initial_owner;
        route_trust_class = route_trust_class_;
        emit OwnershipTransferred(address(0), initial_owner);
    }

    function routeTrustClass() external view returns (bytes32) {
        return route_trust_class;
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setReceiptAcceptance(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 packet_digest,
        bool accepted
    ) external onlyOwner returns (bytes32 receipt_commitment) {
        _requirePftlBytes(source_receipt_root, "source_receipt_root");
        _requirePftlBytes(source_receipt_hash, "source_receipt_hash");
        _requirePftlBytes(route_config_digest, "route_config_digest");
        if (packet_digest == bytes32(0)) {
            revert ZeroPacketDigest();
        }
        receipt_commitment = _receiptCommitment(
            source_receipt_root, source_receipt_hash, route_config_digest, route_trust_class, packet_digest
        );
        accepted_receipt[receipt_commitment] = accepted;
        emit ReceiptAcceptanceSet(receipt_commitment, accepted);
    }

    function isReceiptAccepted(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) external view returns (bool) {
        if (route_trust_class_ != route_trust_class) {
            return false;
        }
        if (packet_digest == bytes32(0)) {
            return false;
        }
        if (source_receipt_root.length != 48 || source_receipt_hash.length != 48 || route_config_digest.length != 48) {
            return false;
        }
        return accepted_receipt[
            _receiptCommitment(
                source_receipt_root, source_receipt_hash, route_config_digest, route_trust_class_, packet_digest
            )
        ];
    }

    function receiptCommitment(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) external pure returns (bytes32) {
        return _receiptCommitment(
            source_receipt_root, source_receipt_hash, route_config_digest, route_trust_class_, packet_digest
        );
    }

    function _receiptCommitment(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) private pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.accepted_receipt.v1",
                source_receipt_root,
                source_receipt_hash,
                route_config_digest,
                route_trust_class_,
                packet_digest
            )
        );
    }

    function _validateTrustClass(bytes32 trust_class) private pure {
        if (trust_class != TRUST_CLASS_CONTROLLED && trust_class != TRUST_CLASS_DISABLED) {
            revert BadTrustClass(trust_class);
        }
    }

    function _requirePftlBytes(bytes calldata value, bytes32 field) private pure {
        if (value.length != 48) {
            revert InvalidPftlBytes(field, value.length, 48);
        }
        for (uint256 i = 0; i < value.length; i++) {
            if (value[i] != 0) {
                return;
            }
        }
        revert ZeroPftlBytes(field);
    }
}

/// @notice Finality verifier for a governed PFTL bridge-signing committee.
/// @dev Committee membership and the exact BFT quorum are immutable for one
///      authority epoch. Rotation deploys a new verifier and drains the old
///      route, so an administrator cannot silently replace the trust root.
contract ThresholdPFTLReceiptVerifier is IPFTLReceiptVerifier {
    error InvalidCommitteeSize(uint256 count);
    error InvalidThreshold(uint256 actual, uint256 required);
    error ZeroAddress(bytes32 field);
    error ZeroDigest(bytes32 field);
    error ZeroHeight();
    error InvalidPftlBytes(bytes32 field, uint256 actual_length, uint256 expected_length);
    error DuplicateOrUnsortedSigner(address signer);
    error InvalidSignatureCount(uint256 actual, uint256 required);
    error BadSignatureLength(uint256 length);
    error BadSignature();
    error UnauthorizedSigner(address signer);
    error ReceiptAlreadyCertified(bytes32 receipt_commitment);
    error ReceiptCodeNotAccepted(bytes32 receipt_code);

    event ReceiptCertified(
        bytes32 indexed receipt_commitment,
        bytes32 indexed packet_digest,
        uint64 indexed finalized_height,
        bytes32 certificate_digest
    );

    uint256 private constant SECP256K1N_HALF = 0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;
    uint256 private constant MAX_COMMITTEE_SIZE = 64;

    bytes32 public constant TRUST_CLASS_BFT_CHECKPOINT = keccak256("BFT_CHECKPOINT");
    bytes32 public constant ACCEPTED_RECEIPT_CODE = keccak256("accepted");

    bytes32 public immutable pftl_chain_id_hash;
    bytes32 public immutable pftl_genesis_hash_commitment;
    uint32 public immutable pftl_protocol_version;
    uint64 public immutable authority_epoch;
    bytes32 public immutable committee_root;
    uint256 public immutable signer_count;
    uint256 public immutable threshold;

    mapping(address => bool) public is_signer;
    mapping(bytes32 => bool) public accepted_receipt;

    constructor(
        bytes32 pftl_chain_id_hash_,
        bytes32 pftl_genesis_hash_commitment_,
        uint32 pftl_protocol_version_,
        uint64 authority_epoch_,
        address[] memory sorted_signers,
        uint256 threshold_
    ) {
        if (pftl_chain_id_hash_ == bytes32(0)) {
            revert ZeroDigest("pftl_chain_id_hash");
        }
        if (pftl_genesis_hash_commitment_ == bytes32(0)) {
            revert ZeroDigest("pftl_genesis_hash_commitment");
        }
        if (pftl_protocol_version_ == 0 || authority_epoch_ == 0) {
            revert ZeroHeight();
        }
        uint256 count = sorted_signers.length;
        if (count == 0 || count > MAX_COMMITTEE_SIZE) {
            revert InvalidCommitteeSize(count);
        }
        uint256 required = count - ((count - 1) / 3);
        if (threshold_ != required) {
            revert InvalidThreshold(threshold_, required);
        }
        address previous = address(0);
        for (uint256 i = 0; i < count; i++) {
            address signer = sorted_signers[i];
            if (signer == address(0)) {
                revert ZeroAddress("signer");
            }
            if (signer <= previous) {
                revert DuplicateOrUnsortedSigner(signer);
            }
            is_signer[signer] = true;
            previous = signer;
        }
        pftl_chain_id_hash = pftl_chain_id_hash_;
        pftl_genesis_hash_commitment = pftl_genesis_hash_commitment_;
        pftl_protocol_version = pftl_protocol_version_;
        authority_epoch = authority_epoch_;
        signer_count = count;
        threshold = threshold_;
        committee_root = keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.bridge_committee.v1",
                pftl_chain_id_hash_,
                pftl_genesis_hash_commitment_,
                pftl_protocol_version_,
                authority_epoch_,
                sorted_signers,
                threshold_
            )
        );
    }

    function routeTrustClass() external pure returns (bytes32) {
        return TRUST_CLASS_BFT_CHECKPOINT;
    }

    function submitReceiptCertificate(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 packet_digest,
        uint64 pftl_finalized_height,
        bytes32 receipt_code,
        bytes[] calldata signatures
    ) external returns (bytes32 receipt_commitment) {
        _validateReceiptFields(
            source_receipt_root,
            source_receipt_hash,
            route_config_digest,
            packet_digest,
            pftl_finalized_height,
            receipt_code
        );
        receipt_commitment = _receiptCommitment(
            source_receipt_root, source_receipt_hash, route_config_digest, TRUST_CLASS_BFT_CHECKPOINT, packet_digest
        );
        if (accepted_receipt[receipt_commitment]) {
            revert ReceiptAlreadyCertified(receipt_commitment);
        }
        bytes32 digest = certificateDigest(
            source_receipt_root,
            source_receipt_hash,
            route_config_digest,
            packet_digest,
            pftl_finalized_height,
            receipt_code
        );
        _validateSignatures(digest, signatures);
        accepted_receipt[receipt_commitment] = true;
        emit ReceiptCertified(receipt_commitment, packet_digest, pftl_finalized_height, digest);
    }

    function isReceiptAccepted(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) external view returns (bool) {
        if (
            route_trust_class_ != TRUST_CLASS_BFT_CHECKPOINT || packet_digest == bytes32(0)
                || source_receipt_root.length != 48 || source_receipt_hash.length != 48
                || route_config_digest.length != 48
        ) {
            return false;
        }
        return accepted_receipt[
            _receiptCommitment(
                source_receipt_root, source_receipt_hash, route_config_digest, route_trust_class_, packet_digest
            )
        ];
    }

    function certificateDigest(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 packet_digest,
        uint64 pftl_finalized_height,
        bytes32 receipt_code
    ) public view returns (bytes32) {
        _validateReceiptFields(
            source_receipt_root,
            source_receipt_hash,
            route_config_digest,
            packet_digest,
            pftl_finalized_height,
            receipt_code
        );
        bytes32 domain_commitment = keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.accepted_receipt_certificate_domain.v1",
                block.chainid,
                address(this),
                pftl_chain_id_hash,
                pftl_genesis_hash_commitment,
                pftl_protocol_version,
                authority_epoch,
                committee_root
            )
        );
        bytes32 evidence_commitment = keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.accepted_receipt_certificate_evidence.v1",
                source_receipt_root,
                source_receipt_hash,
                route_config_digest,
                TRUST_CLASS_BFT_CHECKPOINT,
                packet_digest,
                pftl_finalized_height,
                receipt_code
            )
        );
        return keccak256(
            abi.encode("postfiat.pftl_uniswap.accepted_receipt_certificate.v1", domain_commitment, evidence_commitment)
        );
    }

    function _validateReceiptFields(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 packet_digest,
        uint64 pftl_finalized_height,
        bytes32 receipt_code
    ) private pure {
        _requirePftlBytes(source_receipt_root, "source_receipt_root");
        _requirePftlBytes(source_receipt_hash, "source_receipt_hash");
        _requirePftlBytes(route_config_digest, "route_config_digest");
        if (packet_digest == bytes32(0)) {
            revert ZeroDigest("packet_digest");
        }
        if (pftl_finalized_height == 0) {
            revert ZeroHeight();
        }
        if (receipt_code != ACCEPTED_RECEIPT_CODE) {
            revert ReceiptCodeNotAccepted(receipt_code);
        }
    }

    function _validateSignatures(bytes32 digest, bytes[] calldata signatures) private view {
        if (signatures.length != threshold) {
            revert InvalidSignatureCount(signatures.length, threshold);
        }
        address previous = address(0);
        for (uint256 i = 0; i < signatures.length; i++) {
            address signer = _recover(digest, signatures[i]);
            if (!is_signer[signer]) {
                revert UnauthorizedSigner(signer);
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

    function _receiptCommitment(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) private pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.accepted_receipt.v1",
                source_receipt_root,
                source_receipt_hash,
                route_config_digest,
                route_trust_class_,
                packet_digest
            )
        );
    }

    function _requirePftlBytes(bytes calldata value, bytes32 field) private pure {
        if (value.length != 48) {
            revert InvalidPftlBytes(field, value.length, 48);
        }
        for (uint256 i = 0; i < value.length; i++) {
            if (value[i] != 0) {
                return;
            }
        }
        revert ZeroDigest(field);
    }
}

contract OptimisticPFTLReceiptVerifier is IPFTLReceiptVerifier {
    enum ClaimStatus {
        None,
        Pending,
        Accepted,
        Challenged,
        Rejected
    }

    enum ChallengeFault {
        InvalidReceiptRoot,
        InvalidReceiptHash,
        InvalidRouteConfig,
        InvalidPacketDigest,
        InvalidFinality
    }

    struct ReceiptClaim {
        address poster;
        address challenger;
        uint256 poster_bond_wei;
        uint256 challenger_bond_wei;
        uint64 posted_at;
        uint64 challenge_deadline;
        uint64 challenge_resolution_deadline;
        ChallengeFault challenge_fault;
        bytes32 challenge_evidence_hash;
        bytes32 source_receipt_commitment;
        ClaimStatus status;
    }

    error InvalidPftlBytes(bytes32 field, uint256 actual_length, uint256 expected_length);
    error ZeroPftlBytes(bytes32 field);
    error ZeroPacketDigest();
    error ZeroAddress(bytes32 field);
    error ZeroChallengeEvidence();
    error InvalidChallengeWindow();
    error InsufficientBond(uint256 actual_wei, uint256 required_wei);
    error ClaimAlreadyExists(bytes32 claim_id);
    error UnknownClaim(bytes32 claim_id);
    error InvalidClaimStatus(bytes32 claim_id, ClaimStatus status);
    error ChallengeWindowOpen(uint64 now_timestamp, uint64 challenge_deadline);
    error ChallengeWindowClosed(uint64 now_timestamp, uint64 challenge_deadline);
    error NotOwner(address caller);
    error NotChallengeResolver(address caller);
    error BondTransferFailed(address recipient, uint256 amount_wei);
    error NoBondCredit(address recipient);
    error TimestampOverflow(uint256 timestamp);
    error SourceReceiptClaimAlreadyExists(bytes32 source_receipt_commitment, bytes32 claim_id);

    event ReceiptClaimPosted(
        bytes32 indexed claim_id,
        bytes32 indexed receipt_commitment,
        address indexed poster,
        uint256 bond_wei,
        uint64 challenge_deadline
    );
    event ReceiptClaimChallenged(
        bytes32 indexed claim_id,
        ChallengeFault fault,
        address indexed challenger,
        uint256 bond_wei,
        bytes32 challenge_evidence_hash,
        uint64 resolution_deadline
    );
    event ReceiptChallengeResolved(bytes32 indexed claim_id, bool challenge_valid);
    event ReceiptClaimAccepted(bytes32 indexed claim_id);
    event ReceiptClaimRejected(bytes32 indexed claim_id);
    event BondCreditRecorded(address indexed recipient, uint256 amount_wei);
    event BondCreditWithdrawn(address indexed recipient, uint256 amount_wei);
    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event ChallengeResolverSet(address indexed previous_resolver, address indexed new_resolver);

    bytes32 public constant TRUST_CLASS_OPTIMISTIC = keccak256("OPTIMISTIC");

    address public owner;
    address public challenge_resolver;
    uint256 public immutable poster_bond_wei;
    uint256 public immutable challenger_bond_wei;
    uint64 public immutable challenge_window_seconds;
    uint64 public immutable challenge_resolution_window_seconds;
    mapping(bytes32 => ReceiptClaim) public receipt_claims;
    mapping(bytes32 => bytes32) public source_receipt_claim_id;
    mapping(address => uint256) public bond_credit_wei;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner(msg.sender);
        }
        _;
    }

    modifier onlyChallengeResolver() {
        if (msg.sender != challenge_resolver) {
            revert NotChallengeResolver(msg.sender);
        }
        _;
    }

    constructor(
        address challenge_resolver_,
        uint256 poster_bond_wei_,
        uint256 challenger_bond_wei_,
        uint64 challenge_window_seconds_,
        uint64 challenge_resolution_window_seconds_
    ) {
        if (challenge_resolver_ == address(0)) {
            revert ZeroAddress("challenge_resolver");
        }
        if (poster_bond_wei_ == 0) {
            revert InsufficientBond(0, 1);
        }
        if (challenger_bond_wei_ == 0) {
            revert InsufficientBond(0, 1);
        }
        if (challenge_window_seconds_ == 0 || challenge_resolution_window_seconds_ == 0) {
            revert InvalidChallengeWindow();
        }
        owner = msg.sender;
        challenge_resolver = challenge_resolver_;
        poster_bond_wei = poster_bond_wei_;
        challenger_bond_wei = challenger_bond_wei_;
        challenge_window_seconds = challenge_window_seconds_;
        challenge_resolution_window_seconds = challenge_resolution_window_seconds_;
        emit OwnershipTransferred(address(0), msg.sender);
        emit ChallengeResolverSet(address(0), challenge_resolver_);
    }

    function routeTrustClass() external pure returns (bytes32) {
        return TRUST_CLASS_OPTIMISTIC;
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroAddress("owner");
        }
        address previous_owner = owner;
        owner = new_owner;
        emit OwnershipTransferred(previous_owner, new_owner);
    }

    function setChallengeResolver(address new_resolver) external onlyOwner {
        if (new_resolver == address(0)) {
            revert ZeroAddress("challenge_resolver");
        }
        address previous_resolver = challenge_resolver;
        challenge_resolver = new_resolver;
        emit ChallengeResolverSet(previous_resolver, new_resolver);
    }

    function postReceiptClaim(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 packet_digest
    ) external payable returns (bytes32 claim_id) {
        if (msg.value < poster_bond_wei) {
            revert InsufficientBond(msg.value, poster_bond_wei);
        }
        claim_id = receiptClaimId(source_receipt_root, source_receipt_hash, route_config_digest, packet_digest);
        bytes32 source_receipt_commitment = _sourceReceiptCommitment(source_receipt_root, source_receipt_hash);
        bytes32 existing_source_claim_id = source_receipt_claim_id[source_receipt_commitment];
        if (existing_source_claim_id != bytes32(0)) {
            ClaimStatus existing_status = receipt_claims[existing_source_claim_id].status;
            if (existing_status != ClaimStatus.None && existing_status != ClaimStatus.Rejected) {
                revert SourceReceiptClaimAlreadyExists(source_receipt_commitment, existing_source_claim_id);
            }
        }
        ReceiptClaim storage claim = receipt_claims[claim_id];
        if (claim.status != ClaimStatus.None) {
            revert ClaimAlreadyExists(claim_id);
        }

        uint64 posted_at = _now64();
        uint64 challenge_deadline = posted_at + challenge_window_seconds;
        if (challenge_deadline < posted_at) {
            revert TimestampOverflow(block.timestamp + challenge_window_seconds);
        }

        claim.poster = msg.sender;
        claim.poster_bond_wei = msg.value;
        claim.posted_at = posted_at;
        claim.challenge_deadline = challenge_deadline;
        claim.source_receipt_commitment = source_receipt_commitment;
        claim.status = ClaimStatus.Pending;
        source_receipt_claim_id[source_receipt_commitment] = claim_id;

        emit ReceiptClaimPosted(claim_id, claim_id, msg.sender, msg.value, challenge_deadline);
    }

    function challengeReceiptClaim(bytes32 claim_id, ChallengeFault fault, bytes32 challenge_evidence_hash)
        external
        payable
    {
        if (msg.value < challenger_bond_wei) {
            revert InsufficientBond(msg.value, challenger_bond_wei);
        }
        if (challenge_evidence_hash == bytes32(0)) {
            revert ZeroChallengeEvidence();
        }
        ReceiptClaim storage claim = receipt_claims[claim_id];
        if (claim.status == ClaimStatus.None) {
            revert UnknownClaim(claim_id);
        }
        if (claim.status != ClaimStatus.Pending) {
            revert InvalidClaimStatus(claim_id, claim.status);
        }
        uint64 now_timestamp = _now64();
        if (now_timestamp > claim.challenge_deadline) {
            revert ChallengeWindowClosed(now_timestamp, claim.challenge_deadline);
        }

        claim.status = ClaimStatus.Challenged;
        claim.challenger = msg.sender;
        claim.challenge_fault = fault;
        claim.challenger_bond_wei = msg.value;
        claim.challenge_evidence_hash = challenge_evidence_hash;
        uint64 resolution_deadline = now_timestamp + challenge_resolution_window_seconds;
        if (resolution_deadline < now_timestamp) {
            revert TimestampOverflow(block.timestamp + challenge_resolution_window_seconds);
        }
        if (resolution_deadline < claim.challenge_deadline) {
            resolution_deadline = claim.challenge_deadline;
        }
        claim.challenge_resolution_deadline = resolution_deadline;

        emit ReceiptClaimChallenged(
            claim_id, fault, msg.sender, msg.value, challenge_evidence_hash, claim.challenge_resolution_deadline
        );
    }

    function finalizeReceiptClaim(bytes32 claim_id) external {
        ReceiptClaim storage claim = receipt_claims[claim_id];
        if (claim.status == ClaimStatus.None) {
            revert UnknownClaim(claim_id);
        }
        if (claim.status != ClaimStatus.Pending && claim.status != ClaimStatus.Challenged) {
            revert InvalidClaimStatus(claim_id, claim.status);
        }
        uint64 now_timestamp = _now64();
        if (claim.status == ClaimStatus.Pending && now_timestamp <= claim.challenge_deadline) {
            revert ChallengeWindowOpen(now_timestamp, claim.challenge_deadline);
        }
        if (claim.status == ClaimStatus.Challenged && now_timestamp <= claim.challenge_resolution_deadline) {
            revert ChallengeWindowOpen(now_timestamp, claim.challenge_resolution_deadline);
        }

        if (claim.status == ClaimStatus.Challenged) {
            _rejectClaimAndRefundBonds(claim);
            emit ReceiptClaimRejected(claim_id);
        } else {
            _acceptClaimAndPay(claim_id, claim);
            emit ReceiptClaimAccepted(claim_id);
        }
    }

    function resolveReceiptChallenge(bytes32 claim_id, bool challenge_valid) external onlyChallengeResolver {
        ReceiptClaim storage claim = receipt_claims[claim_id];
        if (claim.status == ClaimStatus.None) {
            revert UnknownClaim(claim_id);
        }
        if (claim.status != ClaimStatus.Challenged) {
            revert InvalidClaimStatus(claim_id, claim.status);
        }
        emit ReceiptChallengeResolved(claim_id, challenge_valid);
        if (challenge_valid) {
            _rejectClaimAndPayChallenger(claim);
            emit ReceiptClaimRejected(claim_id);
        } else {
            _acceptClaimAndPay(claim_id, claim);
            emit ReceiptClaimAccepted(claim_id);
        }
    }

    function withdrawBondCredit() external {
        uint256 amount = bond_credit_wei[msg.sender];
        if (amount == 0) {
            revert NoBondCredit(msg.sender);
        }
        bond_credit_wei[msg.sender] = 0;
        (bool ok,) = msg.sender.call{value: amount}("");
        if (!ok) {
            bond_credit_wei[msg.sender] = amount;
            revert BondTransferFailed(msg.sender, amount);
        }
        emit BondCreditWithdrawn(msg.sender, amount);
    }

    function receiptClaimId(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 packet_digest
    ) public pure returns (bytes32 claim_id) {
        _requirePftlBytes(source_receipt_root, "source_receipt_root");
        _requirePftlBytes(source_receipt_hash, "source_receipt_hash");
        _requirePftlBytes(route_config_digest, "route_config_digest");
        if (packet_digest == bytes32(0)) {
            revert ZeroPacketDigest();
        }
        claim_id = _receiptCommitment(
            source_receipt_root, source_receipt_hash, route_config_digest, TRUST_CLASS_OPTIMISTIC, packet_digest
        );
    }

    function isReceiptAccepted(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) external view returns (bool) {
        if (route_trust_class_ != TRUST_CLASS_OPTIMISTIC) {
            return false;
        }
        if (
            source_receipt_root.length != 48 || source_receipt_hash.length != 48 || route_config_digest.length != 48
                || packet_digest == bytes32(0)
        ) {
            return false;
        }
        bytes32 claim_id = _receiptCommitment(
            source_receipt_root, source_receipt_hash, route_config_digest, route_trust_class_, packet_digest
        );
        return receipt_claims[claim_id].status == ClaimStatus.Accepted;
    }

    function _receiptCommitment(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) private pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.accepted_receipt.v1",
                source_receipt_root,
                source_receipt_hash,
                route_config_digest,
                route_trust_class_,
                packet_digest
            )
        );
    }

    function _sourceReceiptCommitment(bytes calldata source_receipt_root, bytes calldata source_receipt_hash)
        private
        pure
        returns (bytes32)
    {
        return keccak256(
            abi.encode("postfiat.pftl_uniswap.source_receipt.v1", source_receipt_root, source_receipt_hash)
        );
    }

    function _requirePftlBytes(bytes calldata value, bytes32 field) private pure {
        if (value.length != 48) {
            revert InvalidPftlBytes(field, value.length, 48);
        }
        for (uint256 i = 0; i < value.length; i++) {
            if (value[i] != 0) {
                return;
            }
        }
        revert ZeroPftlBytes(field);
    }

    function _now64() private view returns (uint64 timestamp) {
        if (block.timestamp > type(uint64).max) {
            revert TimestampOverflow(block.timestamp);
        }
        timestamp = uint64(block.timestamp);
    }

    function _acceptClaimAndPay(bytes32 claim_id, ReceiptClaim storage claim) private {
        bytes32 active_claim_id = source_receipt_claim_id[claim.source_receipt_commitment];
        if (active_claim_id != claim_id) {
            revert SourceReceiptClaimAlreadyExists(claim.source_receipt_commitment, active_claim_id);
        }
        uint256 payout = claim.poster_bond_wei + claim.challenger_bond_wei;
        address poster = claim.poster;
        claim.status = ClaimStatus.Accepted;
        claim.poster_bond_wei = 0;
        claim.challenger_bond_wei = 0;
        _creditBond(poster, payout);
    }

    function _rejectClaimAndPayChallenger(ReceiptClaim storage claim) private {
        uint256 payout = claim.poster_bond_wei + claim.challenger_bond_wei;
        address challenger = claim.challenger;
        claim.status = ClaimStatus.Rejected;
        claim.poster_bond_wei = 0;
        claim.challenger_bond_wei = 0;
        _creditBond(challenger, payout);
    }

    function _rejectClaimAndRefundBonds(ReceiptClaim storage claim) private {
        uint256 poster_payout = claim.poster_bond_wei;
        uint256 challenger_payout = claim.challenger_bond_wei;
        address poster = claim.poster;
        address challenger = claim.challenger;
        claim.status = ClaimStatus.Rejected;
        claim.poster_bond_wei = 0;
        claim.challenger_bond_wei = 0;
        _creditBond(poster, poster_payout);
        _creditBond(challenger, challenger_payout);
    }

    function _creditBond(address recipient, uint256 amount_wei) private {
        if (amount_wei == 0) {
            return;
        }
        bond_credit_wei[recipient] += amount_wei;
        emit BondCreditRecorded(recipient, amount_wei);
    }
}

contract WrappedVenueNAVCoin {
    error NotOwner();
    error NotController(address caller);
    error ZeroAddress(bytes32 field);
    error ControllerLocked();
    error InvalidAmount();
    error InsufficientBalance(address account, uint256 balance, uint256 amount);
    error InsufficientAllowance(address owner, address spender, uint256 allowance, uint256 amount);

    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);
    event ControllerSet(address indexed controller);
    event ControllerLockedSet(bool locked);
    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);

    string public name;
    string public symbol;
    uint8 public immutable decimals;
    address public owner;
    address public controller;
    bool public controller_locked;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier onlyController() {
        if (msg.sender != controller) {
            revert NotController(msg.sender);
        }
        _;
    }

    constructor(string memory name_, string memory symbol_, uint8 decimals_, address initial_owner) {
        if (initial_owner == address(0)) {
            revert ZeroAddress("initial_owner");
        }
        name = name_;
        symbol = symbol_;
        decimals = decimals_;
        owner = initial_owner;
        emit OwnershipTransferred(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroAddress("new_owner");
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setController(address controller_) external onlyOwner {
        if (controller_locked) {
            revert ControllerLocked();
        }
        if (controller_ == address(0)) {
            revert ZeroAddress("controller");
        }
        controller = controller_;
        emit ControllerSet(controller_);
    }

    function lockController() external onlyOwner {
        controller_locked = true;
        emit ControllerLockedSet(true);
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        _transfer(msg.sender, to, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 current_allowance = allowance[from][msg.sender];
        if (current_allowance < amount) {
            revert InsufficientAllowance(from, msg.sender, current_allowance, amount);
        }
        allowance[from][msg.sender] = current_allowance - amount;
        emit Approval(from, msg.sender, allowance[from][msg.sender]);
        _transfer(from, to, amount);
        return true;
    }

    function mint(address to, uint256 amount) external onlyController {
        if (to == address(0)) {
            revert ZeroAddress("to");
        }
        if (amount == 0) {
            revert InvalidAmount();
        }
        totalSupply += amount;
        balanceOf[to] += amount;
        emit Transfer(address(0), to, amount);
    }

    function burnFromBridge(address from, uint256 amount) external onlyController {
        if (from == address(0)) {
            revert ZeroAddress("from");
        }
        if (amount == 0) {
            revert InvalidAmount();
        }
        uint256 balance = balanceOf[from];
        if (balance < amount) {
            revert InsufficientBalance(from, balance, amount);
        }
        balanceOf[from] = balance - amount;
        totalSupply -= amount;
        emit Transfer(from, address(0), amount);
    }

    function _transfer(address from, address to, uint256 amount) private {
        if (to == address(0)) {
            revert ZeroAddress("to");
        }
        uint256 balance = balanceOf[from];
        if (balance < amount) {
            revert InsufficientBalance(from, balance, amount);
        }
        balanceOf[from] = balance - amount;
        balanceOf[to] += amount;
        emit Transfer(from, to, amount);
    }
}

/// @notice Minimal settlement adapter for the controlled PFTL-Uniswap route.
/// @dev The bridge controller can use this contract as its `IExactInputRouter`.
///      It binds token-in, token-out, pool id, and swap path hash before
///      forwarding to the selected router. It is not an oracle and does not
///      authorize minting by itself.
contract UniswapSettlementAdapter is IExactInputRouter {
    error NotOwner();
    error ZeroAddress(bytes32 field);
    error ControllerLocked();
    error ControllerNotSet();
    error NotController(address caller);
    error PacketConfigMismatch(bytes32 field);
    error TokenTransferFailed();
    error TokenApprovalFailed();
    error RouterOutputBelowMinimum(uint256 amount_out, uint256 minimum_output);
    error RouterBalanceDecreased(uint256 balance_before, uint256 balance_after);
    error DeadlineExpired(uint256 now_timestamp, uint256 deadline);

    event ControllerSet(address indexed controller);
    event ControllerLockedSet(bool locked);
    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event AdapterSwapExecuted(
        address indexed controller,
        address indexed recipient,
        address indexed token_out,
        uint256 amount_in,
        uint256 amount_out,
        bytes32 route_path_hash
    );

    IExactInputRouter public immutable router;
    address public immutable token_in;
    address public immutable token_out;
    bytes32 public immutable uniswap_pool_id;
    bytes32 public immutable route_path_hash;
    address public owner;
    address public controller;
    bool public controller_locked;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    constructor(
        IExactInputRouter router_,
        address token_in_,
        address token_out_,
        bytes32 uniswap_pool_id_,
        bytes32 route_path_hash_,
        address initial_owner
    ) {
        if (address(router_) == address(0)) {
            revert ZeroAddress("router");
        }
        if (token_in_ == address(0)) {
            revert ZeroAddress("token_in");
        }
        if (token_out_ == address(0)) {
            revert ZeroAddress("token_out");
        }
        if (uniswap_pool_id_ == bytes32(0)) {
            revert PacketConfigMismatch("uniswap_pool_id");
        }
        if (route_path_hash_ == bytes32(0)) {
            revert PacketConfigMismatch("route_path_hash");
        }
        if (initial_owner == address(0)) {
            revert ZeroAddress("initial_owner");
        }
        router = router_;
        token_in = token_in_;
        token_out = token_out_;
        uniswap_pool_id = uniswap_pool_id_;
        route_path_hash = route_path_hash_;
        owner = initial_owner;
        emit OwnershipTransferred(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroAddress("new_owner");
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setController(address controller_) external onlyOwner {
        if (controller_locked) {
            revert ControllerLocked();
        }
        if (controller_ == address(0)) {
            revert ZeroAddress("controller");
        }
        controller = controller_;
        emit ControllerSet(controller_);
    }

    function lockController() external onlyOwner {
        controller_locked = true;
        emit ControllerLockedSet(true);
    }

    function exactInput(
        address token_in_,
        address token_out_,
        uint256 amount_in,
        uint256 minimum_output,
        address recipient,
        uint256 deadline,
        bytes calldata data
    ) external returns (uint256 amount_out) {
        if (controller == address(0)) {
            revert ControllerNotSet();
        }
        if (msg.sender != controller) {
            revert NotController(msg.sender);
        }
        if (token_in_ != token_in) {
            revert PacketConfigMismatch("token_in");
        }
        if (token_out_ != token_out) {
            revert PacketConfigMismatch("token_out");
        }
        if (keccak256(data) != route_path_hash) {
            revert PacketConfigMismatch("route_path_hash");
        }
        if (block.timestamp > deadline) {
            revert DeadlineExpired(block.timestamp, deadline);
        }
        if (!IVenueMintableToken(token_in).transferFrom(msg.sender, address(this), amount_in)) {
            revert TokenTransferFailed();
        }
        if (!IVenueMintableToken(token_in).approve(address(router), amount_in)) {
            revert TokenApprovalFailed();
        }
        uint256 balance_before = IERC20Balance(token_out).balanceOf(recipient);
        router.exactInput(token_in, token_out, amount_in, minimum_output, recipient, deadline, data);
        uint256 balance_after = IERC20Balance(token_out).balanceOf(recipient);
        if (balance_after < balance_before) {
            revert RouterBalanceDecreased(balance_before, balance_after);
        }
        amount_out = balance_after - balance_before;
        if (amount_out < minimum_output) {
            revert RouterOutputBelowMinimum(amount_out, minimum_output);
        }
        if (!IVenueMintableToken(token_in).approve(address(router), 0)) {
            revert TokenApprovalFailed();
        }
        emit AdapterSwapExecuted(msg.sender, recipient, token_out, amount_in, amount_out, route_path_hash);
    }
}

contract PacketReplayRegistry {
    error NotOwner();
    error ZeroAddress(bytes32 field);
    error ZeroCommitment(bytes32 field);
    error NotAuthorizedController(address caller);
    error PacketReplay(bytes32 packet_digest);
    error SourcePacketReplay(bytes32 source_packet_commitment);
    error SourceReceiptReplay(bytes32 source_receipt_commitment);
    error CancelledPacketReplay(bytes32 packet_digest);
    error ReturnNonceReplay(bytes32 return_nonce);

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event ControllerAuthorizationSet(address indexed controller, bool authorized);
    event PacketReplayConsumed(
        address indexed controller,
        bytes32 indexed packet_digest,
        bytes32 indexed source_packet_commitment,
        bytes32 source_receipt_commitment
    );
    event PacketReplayCancelled(
        address indexed controller,
        bytes32 indexed packet_digest,
        bytes32 indexed source_packet_commitment,
        bytes32 source_receipt_commitment
    );
    event ReturnNonceConsumed(address indexed controller, bytes32 indexed return_nonce);

    address public owner;
    mapping(address => bool) public authorized_controller;
    mapping(bytes32 => bool) public consumed_packet;
    mapping(bytes32 => bool) public consumed_source_packet;
    mapping(bytes32 => bool) public consumed_source_receipt;
    mapping(bytes32 => bool) public cancelled_packet;
    mapping(bytes32 => bool) public consumed_return_nonce;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier onlyAuthorizedController() {
        if (!authorized_controller[msg.sender]) {
            revert NotAuthorizedController(msg.sender);
        }
        _;
    }

    constructor(address initial_owner) {
        if (initial_owner == address(0)) {
            revert ZeroAddress("initial_owner");
        }
        owner = initial_owner;
        emit OwnershipTransferred(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroAddress("new_owner");
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setControllerAuthorization(address controller, bool authorized) external onlyOwner {
        if (controller == address(0)) {
            revert ZeroAddress("controller");
        }
        authorized_controller[controller] = authorized;
        emit ControllerAuthorizationSet(controller, authorized);
    }

    function consumePacket(bytes32 packet_digest, bytes32 source_packet_commitment, bytes32 source_receipt_commitment)
        external
        onlyAuthorizedController
    {
        _requireCommitment(packet_digest, "packet_digest");
        _requireCommitment(source_packet_commitment, "source_packet_commitment");
        _requireCommitment(source_receipt_commitment, "source_receipt_commitment");
        if (consumed_packet[packet_digest]) {
            revert PacketReplay(packet_digest);
        }
        if (cancelled_packet[packet_digest]) {
            revert CancelledPacketReplay(packet_digest);
        }
        if (consumed_source_packet[source_packet_commitment]) {
            revert SourcePacketReplay(source_packet_commitment);
        }
        if (consumed_source_receipt[source_receipt_commitment]) {
            revert SourceReceiptReplay(source_receipt_commitment);
        }
        consumed_packet[packet_digest] = true;
        consumed_source_packet[source_packet_commitment] = true;
        consumed_source_receipt[source_receipt_commitment] = true;
        emit PacketReplayConsumed(msg.sender, packet_digest, source_packet_commitment, source_receipt_commitment);
    }

    function cancelPacket(bytes32 packet_digest, bytes32 source_packet_commitment, bytes32 source_receipt_commitment)
        external
        onlyAuthorizedController
    {
        _requireCommitment(packet_digest, "packet_digest");
        _requireCommitment(source_packet_commitment, "source_packet_commitment");
        _requireCommitment(source_receipt_commitment, "source_receipt_commitment");
        if (consumed_packet[packet_digest]) {
            revert PacketReplay(packet_digest);
        }
        if (cancelled_packet[packet_digest]) {
            revert CancelledPacketReplay(packet_digest);
        }
        if (consumed_source_packet[source_packet_commitment]) {
            revert SourcePacketReplay(source_packet_commitment);
        }
        if (consumed_source_receipt[source_receipt_commitment]) {
            revert SourceReceiptReplay(source_receipt_commitment);
        }
        cancelled_packet[packet_digest] = true;
        consumed_source_packet[source_packet_commitment] = true;
        consumed_source_receipt[source_receipt_commitment] = true;
        emit PacketReplayCancelled(msg.sender, packet_digest, source_packet_commitment, source_receipt_commitment);
    }

    function consumeReturnNonce(bytes32 return_nonce) external onlyAuthorizedController {
        _requireCommitment(return_nonce, "return_nonce");
        if (consumed_return_nonce[return_nonce]) {
            revert ReturnNonceReplay(return_nonce);
        }
        consumed_return_nonce[return_nonce] = true;
        emit ReturnNonceConsumed(msg.sender, return_nonce);
    }

    function _requireCommitment(bytes32 value, bytes32 field) private pure {
        if (value == bytes32(0)) {
            revert ZeroCommitment(field);
        }
    }
}

/// @notice Controlled-mode PFTL-to-Uniswap handoff controller.
/// @dev This is not a trustless verifier. It consumes route-bound packets only
///      from approved executors and prevents replay, cap drift, config drift,
///      and swap-failure packet consumption.
contract PFTLUniswapHandoffController {
    struct MintAndSwapPacket {
        bytes route_config_digest;
        bytes source_packet_hash;
        bytes source_receipt_hash;
        bytes source_receipt_root;
        uint256 destination_chain_id;
        address destination_bridge;
        address wrapped_navcoin_token;
        bytes32 source_wallet_hash;
        bytes settlement_asset_id;
        bytes native_nav_asset_id;
        bytes pricing_reserve_packet_hash;
        bytes32 uniswap_pool_id;
        bytes32 swap_path_hash;
        address ethereum_recipient;
        address token_out;
        uint256 settlement_amount_atoms;
        uint256 mint_amount_atoms;
        uint256 minimum_output_atoms;
        uint64 pricing_nav_epoch;
        uint64 deadline;
        bytes32 nonce;
    }

    struct RouteConfig {
        address initial_owner;
        uint256 destination_chain_id;
        bytes route_config_digest;
        bytes32 route_trust_class;
        bytes settlement_asset_id;
        bytes native_nav_asset_id;
        bytes pricing_reserve_packet_hash;
        uint64 pricing_nav_epoch;
        bytes32 uniswap_pool_id;
        uint256 route_supply_cap_atoms;
        uint256 packet_notional_cap_atoms;
        address replay_registry;
    }

    error NotOwner();
    error NotApprovedExecutor(address executor);
    error ZeroOwner();
    error ZeroAddress(bytes32 field);
    error ControllerPaused();
    error RouteDisabled();
    error BadTrustClass(bytes32 trust_class);
    error InvalidAmount();
    error DeadlineExpired(uint256 now_timestamp, uint256 deadline);
    error PacketReplay(bytes32 packet_digest);
    error SourcePacketReplay(bytes32 source_packet_commitment);
    error SourceReceiptReplay(bytes32 source_receipt_commitment);
    error ReceiptNotAccepted(bytes32 receipt_commitment);
    error VerifierTrustClassMismatch(bytes32 controller_trust_class, bytes32 verifier_trust_class);
    error PacketConfigMismatch(bytes32 field);
    error InvalidPftlBytes(bytes32 field, uint256 actual_length, uint256 expected_length);
    error PacketNotionalCapExceeded(uint256 attempted_atoms, uint256 cap_atoms);
    error RouteSupplyCapExceeded(uint256 attempted_atoms, uint256 cap_atoms);
    error TokenApprovalFailed();
    error RouterOutputBelowMinimum(uint256 amount_out, uint256 minimum_output);
    error RouterBalanceDecreased(uint256 balance_before, uint256 balance_after);
    error TimestampOverflow(uint256 timestamp);
    error ReturnNonceReplay(bytes32 return_nonce);
    error CancellationBeforeDeadline(uint64 now_timestamp, uint64 deadline);

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event ExecutorApprovalSet(address indexed executor, bool approved);
    event PausedSet(bool paused);
    event PacketConsumed(
        bytes32 indexed packet_digest,
        bytes32 indexed source_packet_commitment,
        address indexed recipient,
        bytes32 route_config_commitment,
        bytes32 source_receipt_commitment,
        bytes32 route_trust_class,
        uint256 mint_amount_atoms,
        uint256 settlement_amount_atoms
    );
    event PacketCancelled(
        bytes32 indexed packet_digest,
        bytes32 indexed source_packet_commitment,
        bytes32 indexed source_receipt_commitment,
        uint64 deadline,
        uint64 cancelled_at
    );
    event WrappedMinted(bytes32 indexed packet_digest, address indexed recipient, uint256 mint_amount_atoms);
    event MintAndSwapExecuted(
        bytes32 indexed packet_digest,
        address indexed recipient,
        address indexed token_out,
        uint256 mint_amount_atoms,
        uint256 amount_out
    );
    event ReturnBurned(
        bytes32 indexed return_burn_id,
        address indexed ethereum_sender,
        bytes32 indexed return_nonce,
        string pftl_recipient,
        bytes native_nav_asset_id,
        uint256 amount_atoms,
        uint256 ethereum_chain_id,
        address bridge_controller,
        address wrapped_navcoin,
        uint256 burn_height
    );

    bytes32 public constant TRUST_CLASS_CONTROLLED = keccak256("CONTROLLED");
    bytes32 public constant TRUST_CLASS_OPTIMISTIC = keccak256("OPTIMISTIC");
    bytes32 public constant TRUST_CLASS_TRUSTLESS_FINALITY = keccak256("TRUSTLESS_FINALITY");
    bytes32 public constant TRUST_CLASS_BFT_CHECKPOINT = keccak256("BFT_CHECKPOINT");
    bytes32 public constant TRUST_CLASS_DISABLED = keccak256("DISABLED");

    IVenueMintableToken public immutable wrapped_navcoin;
    IExactInputRouter public immutable router;
    IPFTLReceiptVerifier public immutable receipt_verifier;
    PacketReplayRegistry public immutable replay_registry;
    bytes32 public immutable route_trust_class;
    bytes32 public immutable uniswap_pool_id;
    uint256 public immutable destination_chain_id;
    uint64 public immutable pricing_nav_epoch;
    uint256 public immutable route_supply_cap_atoms;
    uint256 public immutable packet_notional_cap_atoms;

    address public owner;
    bytes public route_config_digest;
    bytes public settlement_asset_id;
    bytes public native_nav_asset_id;
    bytes public pricing_reserve_packet_hash;
    bool public paused;
    uint256 public total_minted_atoms;
    uint256 public total_settlement_atoms;
    uint256 public total_return_burned_atoms;

    mapping(address => bool) public approved_executor;
    uint256 private reentrancy_lock;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier onlyApprovedExecutor() {
        if (!approved_executor[msg.sender]) {
            revert NotApprovedExecutor(msg.sender);
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
        IVenueMintableToken wrapped_navcoin_,
        IExactInputRouter router_,
        IPFTLReceiptVerifier receipt_verifier_,
        RouteConfig memory config
    ) {
        if (address(wrapped_navcoin_) == address(0)) {
            revert ZeroAddress("wrapped_navcoin");
        }
        if (address(router_) == address(0)) {
            revert ZeroAddress("router");
        }
        if (address(receipt_verifier_) == address(0)) {
            revert ZeroAddress("receipt_verifier");
        }
        if (config.replay_registry == address(0)) {
            revert ZeroAddress("replay_registry");
        }
        if (config.initial_owner == address(0)) {
            revert ZeroOwner();
        }
        if (config.destination_chain_id == 0 || config.destination_chain_id != block.chainid) {
            revert PacketConfigMismatch("destination_chain_id");
        }
        _requirePftlBytesMemory(config.route_config_digest, "route_config_digest");
        _validateTrustClass(config.route_trust_class);
        bytes32 verifier_trust_class = receipt_verifier_.routeTrustClass();
        if (config.route_trust_class != verifier_trust_class) {
            revert VerifierTrustClassMismatch(config.route_trust_class, verifier_trust_class);
        }
        _requirePftlBytesMemory(config.settlement_asset_id, "settlement_asset_id");
        _requirePftlBytesMemory(config.native_nav_asset_id, "native_nav_asset_id");
        _requirePftlBytesMemory(config.pricing_reserve_packet_hash, "pricing_reserve_packet_hash");
        if (config.pricing_nav_epoch == 0) {
            revert InvalidAmount();
        }
        if (config.uniswap_pool_id == bytes32(0)) {
            revert PacketConfigMismatch("uniswap_pool_id");
        }
        if (IPoolBoundExactInputRouter(address(router_)).uniswap_pool_id() != config.uniswap_pool_id) {
            revert PacketConfigMismatch("router_uniswap_pool_id");
        }
        if (config.route_supply_cap_atoms == 0 || config.packet_notional_cap_atoms == 0) {
            revert InvalidAmount();
        }

        wrapped_navcoin = wrapped_navcoin_;
        router = router_;
        receipt_verifier = receipt_verifier_;
        replay_registry = PacketReplayRegistry(config.replay_registry);
        owner = config.initial_owner;
        destination_chain_id = config.destination_chain_id;
        route_config_digest = config.route_config_digest;
        route_trust_class = config.route_trust_class;
        settlement_asset_id = config.settlement_asset_id;
        native_nav_asset_id = config.native_nav_asset_id;
        pricing_reserve_packet_hash = config.pricing_reserve_packet_hash;
        pricing_nav_epoch = config.pricing_nav_epoch;
        uniswap_pool_id = config.uniswap_pool_id;
        route_supply_cap_atoms = config.route_supply_cap_atoms;
        packet_notional_cap_atoms = config.packet_notional_cap_atoms;
        approved_executor[config.initial_owner] = true;

        emit OwnershipTransferred(address(0), config.initial_owner);
        emit ExecutorApprovalSet(config.initial_owner, true);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setExecutorApproval(address executor, bool approved) external onlyOwner {
        if (executor == address(0)) {
            revert ZeroAddress("executor");
        }
        approved_executor[executor] = approved;
        emit ExecutorApprovalSet(executor, approved);
    }

    function setPaused(bool paused_) external onlyOwner {
        paused = paused_;
        emit PausedSet(paused_);
    }

    function verifierTrustClass() external view returns (bytes32) {
        return receipt_verifier.routeTrustClass();
    }

    function outstanding_minted_atoms() public view returns (uint256) {
        return total_minted_atoms - total_return_burned_atoms;
    }

    function consumed_packet(bytes32 packet_digest) external view returns (bool) {
        return replay_registry.consumed_packet(packet_digest);
    }

    function consumed_source_packet(bytes32 source_packet_commitment) external view returns (bool) {
        return replay_registry.consumed_source_packet(source_packet_commitment);
    }

    function consumed_source_receipt(bytes32 source_receipt_commitment) external view returns (bool) {
        return replay_registry.consumed_source_receipt(source_receipt_commitment);
    }

    function consumed_return_nonce(bytes32 return_nonce) external view returns (bool) {
        return replay_registry.consumed_return_nonce(return_nonce);
    }

    function cancelled_packet(bytes32 packet_digest) external view returns (bool) {
        return replay_registry.cancelled_packet(packet_digest);
    }

    function consumeMintOnly(MintAndSwapPacket calldata packet)
        external
        onlyApprovedExecutor
        nonReentrant
        returns (bytes32 packet_digest)
    {
        packet_digest = _consume(packet);
        wrapped_navcoin.mint(packet.ethereum_recipient, packet.mint_amount_atoms);
        emit WrappedMinted(packet_digest, packet.ethereum_recipient, packet.mint_amount_atoms);
    }

    function consumeMintAndSwap(MintAndSwapPacket calldata packet, bytes calldata swap_data)
        external
        onlyApprovedExecutor
        nonReentrant
        returns (bytes32 packet_digest, uint256 amount_out)
    {
        if (packet.swap_path_hash != keccak256(swap_data)) {
            revert PacketConfigMismatch("swap_path_hash");
        }
        packet_digest = _packetDigest(packet);
        _validatePacket(packet, packet_digest);

        wrapped_navcoin.mint(address(this), packet.mint_amount_atoms);
        if (!wrapped_navcoin.approve(address(router), packet.mint_amount_atoms)) {
            revert TokenApprovalFailed();
        }
        uint256 balance_before = IERC20Balance(packet.token_out).balanceOf(packet.ethereum_recipient);
        router.exactInput(
            address(wrapped_navcoin),
            packet.token_out,
            packet.mint_amount_atoms,
            packet.minimum_output_atoms,
            packet.ethereum_recipient,
            packet.deadline,
            swap_data
        );
        uint256 balance_after = IERC20Balance(packet.token_out).balanceOf(packet.ethereum_recipient);
        if (balance_after < balance_before) {
            revert RouterBalanceDecreased(balance_before, balance_after);
        }
        amount_out = balance_after - balance_before;
        if (amount_out < packet.minimum_output_atoms) {
            revert RouterOutputBelowMinimum(amount_out, packet.minimum_output_atoms);
        }

        _markConsumed(packet, packet_digest);
        emit MintAndSwapExecuted(
            packet_digest, packet.ethereum_recipient, packet.token_out, packet.mint_amount_atoms, amount_out
        );
    }

    /// @notice Irreversibly cancels an expired source packet so PFTL can refund it.
    /// @dev The replay registry atomically makes cancellation and consumption
    ///      mutually exclusive. Anyone may relay cancellation after expiry; the
    ///      exact source receipt must already be accepted by the configured verifier.
    function cancelExpiredPacket(MintAndSwapPacket calldata packet)
        external
        nonReentrant
        returns (bytes32 packet_digest)
    {
        packet_digest = _packetDigest(packet);
        _validatePacketReplay(packet, packet_digest);
        _validatePacketBoundConfig(packet, packet_digest);
        _validatePacketAmounts(packet);
        uint64 now_timestamp = _now64();
        if (now_timestamp <= packet.deadline) {
            revert CancellationBeforeDeadline(now_timestamp, packet.deadline);
        }
        bytes32 source_packet_commitment = _sourcePacketCommitment(packet.source_packet_hash);
        bytes32 source_receipt_commitment = _sourceReceiptReplayCommitment(packet);
        replay_registry.cancelPacket(packet_digest, source_packet_commitment, source_receipt_commitment);
        emit PacketCancelled(
            packet_digest, source_packet_commitment, source_receipt_commitment, packet.deadline, now_timestamp
        );
    }

    function burnForPftlReturn(
        uint256 amount_atoms,
        string calldata pftl_recipient,
        bytes calldata destination_native_nav_asset_id,
        bytes32 return_nonce
    ) external nonReentrant returns (bytes32 return_burn_id) {
        if (route_trust_class == TRUST_CLASS_DISABLED) {
            revert RouteDisabled();
        }
        if (amount_atoms == 0) {
            revert InvalidAmount();
        }
        if (bytes(pftl_recipient).length == 0) {
            revert PacketConfigMismatch("pftl_recipient");
        }
        if (return_nonce == bytes32(0)) {
            revert PacketConfigMismatch("return_nonce");
        }
        if (replay_registry.consumed_return_nonce(return_nonce)) {
            revert ReturnNonceReplay(return_nonce);
        }
        _requirePftlBytesCalldata(destination_native_nav_asset_id, "destination_native_nav_asset_id");
        if (keccak256(destination_native_nav_asset_id) != keccak256(native_nav_asset_id)) {
            revert PacketConfigMismatch("destination_native_nav_asset_id");
        }

        return_burn_id = keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.return_burn.v1",
                block.chainid,
                address(this),
                address(wrapped_navcoin),
                destination_native_nav_asset_id,
                msg.sender,
                pftl_recipient,
                amount_atoms,
                return_nonce,
                block.number
            )
        );
        replay_registry.consumeReturnNonce(return_nonce);
        total_return_burned_atoms += amount_atoms;
        wrapped_navcoin.burnFromBridge(msg.sender, amount_atoms);
        emit ReturnBurned(
            return_burn_id,
            msg.sender,
            return_nonce,
            pftl_recipient,
            destination_native_nav_asset_id,
            amount_atoms,
            block.chainid,
            address(this),
            address(wrapped_navcoin),
            block.number
        );
    }

    function packetDigest(MintAndSwapPacket calldata packet) external pure returns (bytes32) {
        return _packetDigest(packet);
    }

    function _consume(MintAndSwapPacket calldata packet) private returns (bytes32 packet_digest) {
        packet_digest = _packetDigest(packet);
        _validatePacket(packet, packet_digest);
        _markConsumed(packet, packet_digest);
    }

    function _validatePacket(MintAndSwapPacket calldata packet, bytes32 packet_digest) private view {
        if (paused) {
            revert ControllerPaused();
        }
        if (route_trust_class == TRUST_CLASS_DISABLED) {
            revert RouteDisabled();
        }
        _validatePacketReplay(packet, packet_digest);
        _validatePacketBoundConfig(packet, packet_digest);
        _validatePacketAmountsAndDeadline(packet);
    }

    function _validatePacketReplay(MintAndSwapPacket calldata packet, bytes32 packet_digest) private view {
        if (replay_registry.consumed_packet(packet_digest)) {
            revert PacketReplay(packet_digest);
        }
        if (replay_registry.cancelled_packet(packet_digest)) {
            revert PacketReplay(packet_digest);
        }
        bytes32 source_packet_commitment = _sourcePacketCommitment(packet.source_packet_hash);
        if (replay_registry.consumed_source_packet(source_packet_commitment)) {
            revert SourcePacketReplay(source_packet_commitment);
        }
        bytes32 source_receipt_commitment = _sourceReceiptReplayCommitment(packet);
        if (replay_registry.consumed_source_receipt(source_receipt_commitment)) {
            revert SourceReceiptReplay(source_receipt_commitment);
        }
    }

    function _validatePacketBoundConfig(MintAndSwapPacket calldata packet, bytes32 packet_digest) private view {
        _requirePftlBytesCalldata(packet.route_config_digest, "route_config_digest");
        _requirePftlBytesCalldata(packet.source_packet_hash, "source_packet_hash");
        _requirePftlBytesCalldata(packet.source_receipt_hash, "source_receipt_hash");
        _requirePftlBytesCalldata(packet.source_receipt_root, "source_receipt_root");
        _requirePftlBytesCalldata(packet.settlement_asset_id, "settlement_asset_id");
        _requirePftlBytesCalldata(packet.native_nav_asset_id, "native_nav_asset_id");
        _requirePftlBytesCalldata(packet.pricing_reserve_packet_hash, "pricing_reserve_packet_hash");
        if (keccak256(packet.route_config_digest) != keccak256(route_config_digest)) {
            revert PacketConfigMismatch("route_config_digest");
        }
        if (packet.destination_chain_id != destination_chain_id || packet.destination_chain_id != block.chainid) {
            revert PacketConfigMismatch("destination_chain_id");
        }
        if (packet.destination_bridge != address(this)) {
            revert PacketConfigMismatch("destination_bridge");
        }
        if (packet.wrapped_navcoin_token != address(wrapped_navcoin)) {
            revert PacketConfigMismatch("wrapped_navcoin_token");
        }
        if (keccak256(packet.settlement_asset_id) != keccak256(settlement_asset_id)) {
            revert PacketConfigMismatch("settlement_asset_id");
        }
        if (keccak256(packet.native_nav_asset_id) != keccak256(native_nav_asset_id)) {
            revert PacketConfigMismatch("native_nav_asset_id");
        }
        if (packet.pricing_nav_epoch != pricing_nav_epoch) {
            revert PacketConfigMismatch("pricing_nav_epoch");
        }
        if (keccak256(packet.pricing_reserve_packet_hash) != keccak256(pricing_reserve_packet_hash)) {
            revert PacketConfigMismatch("pricing_reserve_packet_hash");
        }
        if (packet.uniswap_pool_id != uniswap_pool_id) {
            revert PacketConfigMismatch("uniswap_pool_id");
        }
        if (!receipt_verifier.isReceiptAccepted(
                packet.source_receipt_root,
                packet.source_receipt_hash,
                packet.route_config_digest,
                route_trust_class,
                packet_digest
            )) {
            revert ReceiptNotAccepted(_acceptedReceiptCommitment(packet, packet_digest));
        }
    }

    function _validatePacketAmountsAndDeadline(MintAndSwapPacket calldata packet) private view {
        _validatePacketAmounts(packet);
        uint64 now_timestamp = _now64();
        if (now_timestamp > packet.deadline) {
            revert DeadlineExpired(now_timestamp, packet.deadline);
        }
    }

    function _validatePacketAmounts(MintAndSwapPacket calldata packet) private view {
        if (packet.ethereum_recipient == address(0)) {
            revert ZeroAddress("ethereum_recipient");
        }
        if (
            packet.settlement_amount_atoms == 0 || packet.mint_amount_atoms == 0 || packet.minimum_output_atoms == 0
                || packet.pricing_nav_epoch == 0
        ) {
            revert InvalidAmount();
        }
        if (packet.settlement_amount_atoms > packet_notional_cap_atoms) {
            revert PacketNotionalCapExceeded(packet.settlement_amount_atoms, packet_notional_cap_atoms);
        }
        uint256 next_outstanding = outstanding_minted_atoms() + packet.mint_amount_atoms;
        if (next_outstanding > route_supply_cap_atoms) {
            revert RouteSupplyCapExceeded(next_outstanding, route_supply_cap_atoms);
        }
        if (packet.nonce == bytes32(0)) {
            revert PacketConfigMismatch("packet_hash_field");
        }
    }

    function _markConsumed(MintAndSwapPacket calldata packet, bytes32 packet_digest) private {
        bytes32 source_packet_commitment = _sourcePacketCommitment(packet.source_packet_hash);
        bytes32 source_receipt_commitment = _sourceReceiptReplayCommitment(packet);
        replay_registry.consumePacket(packet_digest, source_packet_commitment, source_receipt_commitment);
        total_minted_atoms += packet.mint_amount_atoms;
        total_settlement_atoms += packet.settlement_amount_atoms;
        emit PacketConsumed(
            packet_digest,
            source_packet_commitment,
            packet.ethereum_recipient,
            keccak256(packet.route_config_digest),
            source_receipt_commitment,
            route_trust_class,
            packet.mint_amount_atoms,
            packet.settlement_amount_atoms
        );
    }

    function _packetDigest(MintAndSwapPacket calldata packet) private pure returns (bytes32) {
        return keccak256(abi.encode("postfiat.pftl_uniswap.mint_and_swap_packet.v1", packet));
    }

    function _validateTrustClass(bytes32 trust_class) private pure {
        if (
            trust_class != TRUST_CLASS_CONTROLLED && trust_class != TRUST_CLASS_OPTIMISTIC
                && trust_class != TRUST_CLASS_TRUSTLESS_FINALITY && trust_class != TRUST_CLASS_BFT_CHECKPOINT
                && trust_class != TRUST_CLASS_DISABLED
        ) {
            revert BadTrustClass(trust_class);
        }
    }

    function _sourcePacketCommitment(bytes calldata source_packet_hash) private pure returns (bytes32) {
        return keccak256(source_packet_hash);
    }

    function _sourceReceiptReplayCommitment(MintAndSwapPacket calldata packet) private pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.source_receipt.v1", packet.source_receipt_root, packet.source_receipt_hash
            )
        );
    }

    function _acceptedReceiptCommitment(MintAndSwapPacket calldata packet, bytes32 packet_digest)
        private
        view
        returns (bytes32)
    {
        return _receiptCommitment(
            packet.source_receipt_root,
            packet.source_receipt_hash,
            packet.route_config_digest,
            route_trust_class,
            packet_digest
        );
    }

    function _receiptCommitment(
        bytes calldata source_receipt_root,
        bytes calldata source_receipt_hash,
        bytes calldata route_config_digest_,
        bytes32 route_trust_class_,
        bytes32 packet_digest
    ) private pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.accepted_receipt.v1",
                source_receipt_root,
                source_receipt_hash,
                route_config_digest_,
                route_trust_class_,
                packet_digest
            )
        );
    }

    function _requirePftlBytesMemory(bytes memory value, bytes32 field) private pure {
        if (value.length != 48) {
            revert InvalidPftlBytes(field, value.length, 48);
        }
        for (uint256 i = 0; i < value.length; i++) {
            if (value[i] != 0) {
                return;
            }
        }
        revert PacketConfigMismatch(field);
    }

    function _requirePftlBytesCalldata(bytes calldata value, bytes32 field) private pure {
        if (value.length != 48) {
            revert InvalidPftlBytes(field, value.length, 48);
        }
        for (uint256 i = 0; i < value.length; i++) {
            if (value[i] != 0) {
                return;
            }
        }
        revert PacketConfigMismatch(field);
    }

    function _now64() private view returns (uint64) {
        if (block.timestamp > type(uint64).max) {
            revert TimestampOverflow(block.timestamp);
        }
        return uint64(block.timestamp);
    }
}
