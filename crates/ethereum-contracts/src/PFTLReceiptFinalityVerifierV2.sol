// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IPFTLReceiptSP1VerifierV2 {
    function verifyProof(bytes32 programVKey, bytes calldata publicValues, bytes calldata proofBytes) external view;
}

/// @notice Proof-native, ownerless verifier for PFTL primary-mint receipts.
///
/// Successor to PFTLReceiptFinalityVerifierV1. V1 carried defect P0-01: the SP1
/// guest took the validator committee from the prover's own input with no
/// ancestry, and V1 only checked that the committee commitment was nonzero
/// (`PFTLReceiptFinalityVerifierV1.sol:302-303`). V1 also recorded EVERY
/// resulting checkpoint as accepted, even one that did not advance the tip
/// (`_recordCheckpoint`, :320-328), while one acceptance path required only that
/// the prior checkpoint be somewhere in that ever-growing set (:182). Together
/// those let a made-up committee plus a made-up checkpoint below the tip mint.
///
/// This contract keeps three concerns separate, because conflating them is how
/// P0-01 survived review:
///
///   1. ANCESTRY     - the committee root must come from a proven chain rooted at
///                     the pinned genesis anchor, never from caller input.
///   2. ORDERING     - advancing the stored tip requires a strictly greater
///                     height. Ordering is not evidence of ancestry.
///   3. REPLAY       - a route-scoped receipt identifier, independent of the
///                     checkpoint proved against, is consumed exactly once.
///                     Ancestry and ordering are not replay prevention.
///
/// The contract is ownerless. There is no path to widen or replace the pinned
/// verifying key, the ancestry anchor or the ancestry rules. Upgrades, if ever
/// wanted, are a separate governed contract reviewed on its own.
contract PFTLReceiptFinalityVerifierV2 {
    struct Config {
        IPFTLReceiptSP1VerifierV2 sp1Verifier;
        bytes32 programVKey;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        bytes32 routeIdCommitment;
        bytes32 nativeNavAssetIdCommitment;
        bytes32 settlementAssetIdCommitment;
        uint256 destinationChainId;
        address controller;
        address wrappedToken;
        bytes32 wrappedTokenRuntimeCodeHash;
        uint256 maxProofBytes;
        uint256 maxPublicValuesBytes;
        /// Ancestry anchor. Every accepted proof must prove a chain rooted here.
        bytes32 ancestryAnchorCommitment;
        uint64 ancestryAnchorHeight;
    }

    /// @dev Public values committed by the ancestry-proving guest program.
    ///      Added over V1: `ancestryRootCommitment`, `ancestrySegmentCommitment`,
    ///      `provenCommitteeRootCommitment`, `receiptIdentifier` and
    ///      `multiplierPolicyCommitment`.
    struct ReceiptPublicValues {
        uint32 proofProgramVersion;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        /// The anchor the proven ancestry chain is rooted at.
        bytes32 ancestryRootCommitment;
        /// Commitment to the proven chain from the anchor to the claimed checkpoint.
        bytes32 ancestrySegmentCommitment;
        /// Committee root as derived inside the proof from the proven segment.
        bytes32 provenCommitteeRootCommitment;
        /// Caller-visible committee root. Must equal the proven one exactly.
        bytes32 committeeRootCommitment;
        bytes32 committeeTransitionCommitment;
        bytes32 finalizedBlockCommitment;
        bytes32 finalizedStateRootCommitment;
        uint64 routeEpoch;
        bytes32 policyHashCommitment;
        bytes32 routeIdCommitment;
        bytes32 routeTrustClass;
        bytes32 routeConfigDigestCommitment;
        /// Commitment to the fee multipliers, bound into the route config digest.
        bytes32 multiplierPolicyCommitment;
        bytes32 nativeNavAssetIdCommitment;
        bytes32 settlementAssetIdCommitment;
        uint64 pricingNavEpoch;
        bytes32 pricingReservePacketHashCommitment;
        bytes32 sourceWalletCommitment;
        bytes32 sourceReceiptRootCommitment;
        bytes32 sourceReceiptHashCommitment;
        bytes32 acceptedReceiptCode;
        bytes32 packetDigest;
        uint256 destinationChainId;
        address controller;
        address wrappedToken;
        address recipient;
        uint256 mintAmountAtoms;
        uint256 settlementValueAtoms;
        bytes32 packetNonce;
        uint64 deadline;
        uint64 sourceHeight;
        bytes32 priorCheckpointCommitment;
        bytes32 resultingCheckpointCommitment;
        uint64 finalizedHeight;
        /// Route-scoped, checkpoint-independent receipt identity.
        bytes32 receiptIdentifier;
        bytes32 proofNullifier;
    }

    struct CheckpointPublicValues {
        uint32 proofProgramVersion;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        bytes32 ancestryRootCommitment;
        bytes32 ancestrySegmentCommitment;
        bytes32 provenCommitteeRootCommitment;
        bytes32 committeeRootCommitment;
        bytes32 priorCheckpointCommitment;
        bytes32 resultingCheckpointCommitment;
        uint64 finalizedHeight;
        bytes32 proofNullifier;
    }

    error ZeroAddress(bytes32 field);
    error ZeroValue(bytes32 field);
    error WrongBinding(bytes32 field);
    error ProofTooLarge(uint256 actual, uint256 maximum);
    error PublicValuesTooLarge(uint256 actual, uint256 maximum);
    error NonCanonicalPublicValues();
    /// Ancestry failures are their own error class so they cannot be read as ordering.
    error WrongAncestryRoot(bytes32 claimed, bytes32 pinned);
    error AbsentAncestrySegment();
    error CommitteeRootNotProven(bytes32 supplied, bytes32 proven);
    error UnknownPriorCheckpoint(bytes32 checkpoint);
    error StaleCheckpoint(uint64 actual, uint64 pinnedTip);
    error ProofAlreadyConsumed(bytes32 nullifier);
    error ReceiptAlreadyConsumed(bytes32 receiptIdentifier);

    event ReceiptAccepted(
        bytes32 indexed receiptIdentifier,
        bytes32 indexed packetDigest,
        bytes32 indexed proofNullifier,
        address recipient,
        uint256 mintAmountAtoms,
        uint64 finalizedHeight
    );
    event CheckpointAdvanced(
        bytes32 indexed priorCheckpointCommitment,
        bytes32 indexed resultingCheckpointCommitment,
        uint64 finalizedHeight
    );

    bytes32 public constant TRUST_CLASS_TRUSTLESS_FINALITY = keccak256("TRUSTLESS_FINALITY");
    bytes32 public constant ACCEPTED_EXPORT_RECEIPT_CODE = keccak256("export_debit");

    IPFTLReceiptSP1VerifierV2 public immutable sp1Verifier;
    bytes32 public immutable programVKey;
    bytes32 public immutable pftlChainIdHash;
    bytes32 public immutable pftlGenesisHashCommitment;
    uint32 public immutable pftlProtocolVersion;
    bytes32 public immutable routeIdCommitment;
    bytes32 public immutable nativeNavAssetIdCommitment;
    bytes32 public immutable settlementAssetIdCommitment;
    uint256 public immutable destinationChainId;
    address public immutable controller;
    address public immutable wrappedToken;
    bytes32 public immutable wrappedTokenRuntimeCodeHash;
    uint256 public immutable maxProofBytes;
    uint256 public immutable maxPublicValuesBytes;
    /// Immutable ancestry anchor. No function can change these.
    bytes32 public immutable ancestryAnchorCommitment;
    uint64 public immutable ancestryAnchorHeight;

    /// The single stored tip. Ordering is judged against this and nothing else.
    bytes32 public pinnedTipCommitment;
    uint64 public pinnedTipHeight;
    /// Height at which a checkpoint became part of the canonical chain. Only
    /// written for checkpoints that advanced the tip, so this mapping can never
    /// be grown by a rejected or non-advancing claim -- the V1 mistake.
    mapping(bytes32 => uint64) public canonicalCheckpointHeight;
    mapping(bytes32 => bool) public consumedProofNullifier;
    mapping(bytes32 => bool) public consumedReceiptIdentifier;

    constructor(Config memory config) {
        if (address(config.sp1Verifier) == address(0)) revert ZeroAddress("sp1_verifier");
        if (config.controller == address(0)) revert ZeroAddress("controller");
        if (config.wrappedToken == address(0)) revert ZeroAddress("wrapped_token");
        if (
            config.programVKey == bytes32(0) || config.pftlChainIdHash == bytes32(0)
                || config.pftlGenesisHashCommitment == bytes32(0) || config.pftlProtocolVersion == 0
                || config.routeIdCommitment == bytes32(0)
                || config.nativeNavAssetIdCommitment == bytes32(0)
                || config.settlementAssetIdCommitment == bytes32(0)
                || config.destinationChainId == 0 || config.wrappedTokenRuntimeCodeHash == bytes32(0)
                || config.maxProofBytes == 0 || config.maxPublicValuesBytes == 0
                || config.ancestryAnchorCommitment == bytes32(0) || config.ancestryAnchorHeight == 0
        ) revert ZeroValue("constructor");
        if (config.destinationChainId != block.chainid) revert WrongBinding("destination_chain_id");
        if (config.wrappedToken.codehash != config.wrappedTokenRuntimeCodeHash) {
            revert WrongBinding("wrapped_token_code_hash");
        }

        sp1Verifier = config.sp1Verifier;
        programVKey = config.programVKey;
        pftlChainIdHash = config.pftlChainIdHash;
        pftlGenesisHashCommitment = config.pftlGenesisHashCommitment;
        pftlProtocolVersion = config.pftlProtocolVersion;
        routeIdCommitment = config.routeIdCommitment;
        nativeNavAssetIdCommitment = config.nativeNavAssetIdCommitment;
        settlementAssetIdCommitment = config.settlementAssetIdCommitment;
        destinationChainId = config.destinationChainId;
        controller = config.controller;
        wrappedToken = config.wrappedToken;
        wrappedTokenRuntimeCodeHash = config.wrappedTokenRuntimeCodeHash;
        maxProofBytes = config.maxProofBytes;
        maxPublicValuesBytes = config.maxPublicValuesBytes;
        ancestryAnchorCommitment = config.ancestryAnchorCommitment;
        ancestryAnchorHeight = config.ancestryAnchorHeight;

        pinnedTipCommitment = config.ancestryAnchorCommitment;
        pinnedTipHeight = config.ancestryAnchorHeight;
        canonicalCheckpointHeight[config.ancestryAnchorCommitment] = config.ancestryAnchorHeight;
    }

    function routeTrustClass() external pure returns (bytes32) {
        return TRUST_CLASS_TRUSTLESS_FINALITY;
    }

    /// @notice Accept one export receipt. Mints are the controller's business;
    ///         this contract decides only whether the receipt is admissible.
    function verifyAndAccept(bytes calldata publicValues, bytes calldata proofBytes)
        external
        returns (bytes32 receiptIdentifier)
    {
        _requireBounds(publicValues, proofBytes);
        ReceiptPublicValues memory decoded = abi.decode(publicValues, (ReceiptPublicValues));
        if (keccak256(publicValues) != keccak256(abi.encode(decoded))) revert NonCanonicalPublicValues();

        _requireReceiptBindings(decoded);
        // (1) ANCESTRY, before anything is trusted.
        _requireAncestry(
            decoded.ancestryRootCommitment,
            decoded.ancestrySegmentCommitment,
            decoded.provenCommitteeRootCommitment,
            decoded.committeeRootCommitment
        );
        // The prior checkpoint must be canonical, not merely "seen before".
        if (canonicalCheckpointHeight[decoded.priorCheckpointCommitment] == 0) {
            revert UnknownPriorCheckpoint(decoded.priorCheckpointCommitment);
        }
        // (3) REPLAY, on a route-scoped identifier independent of the checkpoint.
        if (consumedReceiptIdentifier[decoded.receiptIdentifier]) {
            revert ReceiptAlreadyConsumed(decoded.receiptIdentifier);
        }
        if (consumedProofNullifier[decoded.proofNullifier]) {
            revert ProofAlreadyConsumed(decoded.proofNullifier);
        }

        sp1Verifier.verifyProof(programVKey, publicValues, proofBytes);

        consumedProofNullifier[decoded.proofNullifier] = true;
        consumedReceiptIdentifier[decoded.receiptIdentifier] = true;
        // (2) ORDERING, separately, and only ever forward.
        _advanceTipIfStrictlyGreater(
            decoded.priorCheckpointCommitment,
            decoded.resultingCheckpointCommitment,
            decoded.finalizedHeight
        );

        receiptIdentifier = decoded.receiptIdentifier;
        emit ReceiptAccepted(
            receiptIdentifier,
            decoded.packetDigest,
            decoded.proofNullifier,
            decoded.recipient,
            decoded.mintAmountAtoms,
            decoded.finalizedHeight
        );
    }

    /// @notice Advance the stored tip. Requires ancestry AND a strictly greater height.
    function advanceCheckpoint(bytes calldata publicValues, bytes calldata proofBytes) external {
        _requireBounds(publicValues, proofBytes);
        CheckpointPublicValues memory decoded = abi.decode(publicValues, (CheckpointPublicValues));
        if (keccak256(publicValues) != keccak256(abi.encode(decoded))) revert NonCanonicalPublicValues();
        if (
            decoded.proofProgramVersion != 2 || decoded.pftlChainIdHash != pftlChainIdHash
                || decoded.pftlGenesisHashCommitment != pftlGenesisHashCommitment
                || decoded.pftlProtocolVersion != pftlProtocolVersion
                || decoded.resultingCheckpointCommitment == bytes32(0)
                || decoded.proofNullifier == bytes32(0)
        ) revert WrongBinding("checkpoint");

        _requireAncestry(
            decoded.ancestryRootCommitment,
            decoded.ancestrySegmentCommitment,
            decoded.provenCommitteeRootCommitment,
            decoded.committeeRootCommitment
        );
        if (decoded.priorCheckpointCommitment != pinnedTipCommitment) {
            revert UnknownPriorCheckpoint(decoded.priorCheckpointCommitment);
        }
        // Strictly greater. Equal or below is rejected, and rejection reverts, so
        // the stored tip is untouched.
        if (decoded.finalizedHeight <= pinnedTipHeight) {
            revert StaleCheckpoint(decoded.finalizedHeight, pinnedTipHeight);
        }
        if (consumedProofNullifier[decoded.proofNullifier]) {
            revert ProofAlreadyConsumed(decoded.proofNullifier);
        }

        sp1Verifier.verifyProof(programVKey, publicValues, proofBytes);

        consumedProofNullifier[decoded.proofNullifier] = true;
        _advanceTipIfStrictlyGreater(
            decoded.priorCheckpointCommitment,
            decoded.resultingCheckpointCommitment,
            decoded.finalizedHeight
        );
    }

    function isReceiptConsumed(bytes32 receiptIdentifier) external view returns (bool) {
        return consumedReceiptIdentifier[receiptIdentifier];
    }

    /// @dev ANCESTRY. The committee root is whatever the proof derived from the
    ///      proven segment; a caller-supplied value is only ever compared, never
    ///      used. This is the direct remedy for P0-01.
    function _requireAncestry(
        bytes32 ancestryRootCommitment,
        bytes32 ancestrySegmentCommitment,
        bytes32 provenCommitteeRootCommitment,
        bytes32 suppliedCommitteeRootCommitment
    ) private view {
        if (ancestryRootCommitment != ancestryAnchorCommitment) {
            revert WrongAncestryRoot(ancestryRootCommitment, ancestryAnchorCommitment);
        }
        if (ancestrySegmentCommitment == bytes32(0)) revert AbsentAncestrySegment();
        if (provenCommitteeRootCommitment == bytes32(0)) revert CommitteeRootNotProven(suppliedCommitteeRootCommitment, provenCommitteeRootCommitment);
        if (suppliedCommitteeRootCommitment != provenCommitteeRootCommitment) {
            revert CommitteeRootNotProven(suppliedCommitteeRootCommitment, provenCommitteeRootCommitment);
        }
    }

    /// @dev ORDERING. Records a checkpoint as canonical only when it advances the
    ///      tip. V1 recorded every resulting checkpoint unconditionally, which is
    ///      what let a made-up checkpoint below the tip become a valid `prior`.
    function _advanceTipIfStrictlyGreater(bytes32 prior, bytes32 resulting, uint64 finalizedHeight) private {
        if (resulting == bytes32(0)) revert ZeroValue("resulting_checkpoint");
        if (finalizedHeight <= pinnedTipHeight) {
            return;
        }
        pinnedTipCommitment = resulting;
        pinnedTipHeight = finalizedHeight;
        canonicalCheckpointHeight[resulting] = finalizedHeight;
        emit CheckpointAdvanced(prior, resulting, finalizedHeight);
    }

    function _requireReceiptBindings(ReceiptPublicValues memory decoded) private view {
        if (
            decoded.proofProgramVersion != 2 || decoded.pftlChainIdHash != pftlChainIdHash
                || decoded.pftlGenesisHashCommitment != pftlGenesisHashCommitment
                || decoded.pftlProtocolVersion != pftlProtocolVersion
                || decoded.routeIdCommitment != routeIdCommitment
                || decoded.nativeNavAssetIdCommitment != nativeNavAssetIdCommitment
                || decoded.settlementAssetIdCommitment != settlementAssetIdCommitment
                || decoded.routeTrustClass != TRUST_CLASS_TRUSTLESS_FINALITY
                || decoded.acceptedReceiptCode != ACCEPTED_EXPORT_RECEIPT_CODE
                || decoded.destinationChainId != destinationChainId
                || decoded.destinationChainId != block.chainid
                || decoded.controller != controller || decoded.wrappedToken != wrappedToken
                || decoded.packetDigest == bytes32(0) || decoded.recipient == address(0)
                || decoded.mintAmountAtoms == 0 || decoded.settlementValueAtoms == 0
                || decoded.packetNonce == bytes32(0) || decoded.deadline == 0
                || decoded.sourceHeight == 0
                || decoded.resultingCheckpointCommitment == bytes32(0)
                || decoded.proofNullifier == bytes32(0)
                || decoded.receiptIdentifier == bytes32(0)
        ) revert WrongBinding("receipt");
        if (
            decoded.routeConfigDigestCommitment == bytes32(0)
                || decoded.multiplierPolicyCommitment == bytes32(0)
                || decoded.committeeRootCommitment == bytes32(0)
                || decoded.committeeTransitionCommitment == bytes32(0)
                || decoded.finalizedBlockCommitment == bytes32(0)
                || decoded.finalizedStateRootCommitment == bytes32(0)
                || decoded.pricingNavEpoch == 0
                || decoded.pricingReservePacketHashCommitment == bytes32(0)
                || decoded.sourceWalletCommitment == bytes32(0)
                || decoded.sourceReceiptRootCommitment == bytes32(0)
                || decoded.sourceReceiptHashCommitment == bytes32(0)
        ) revert WrongBinding("receipt_commitments");
        if (controller.code.length == 0) revert WrongBinding("controller_code");
        if (wrappedToken.codehash != wrappedTokenRuntimeCodeHash) revert WrongBinding("wrapped_token_code_hash");
    }

    function _requireBounds(bytes calldata publicValues, bytes calldata proofBytes) private view {
        if (proofBytes.length == 0 || proofBytes.length > maxProofBytes) {
            revert ProofTooLarge(proofBytes.length, maxProofBytes);
        }
        if (publicValues.length == 0 || publicValues.length > maxPublicValuesBytes) {
            revert PublicValuesTooLarge(publicValues.length, maxPublicValuesBytes);
        }
    }
}
