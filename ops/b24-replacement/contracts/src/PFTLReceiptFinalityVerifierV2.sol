// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IPFTLReceiptSP1VerifierV2 {
    function verifyProof(bytes32 programVKey, bytes calldata publicValues, bytes calldata proofBytes) external view;
}

/// @notice Proof-native, ownerless verifier for PFTL primary-mint receipts.
/// @dev The SP1 guest proves consensus finality and membership of the exact
///      route receipt in the finalized receipt root. Every deployment pins one
///      chain, program, route epoch, policy, controller, and wrapped token.
contract PFTLReceiptFinalityVerifierV2 {
    struct Config {
        IPFTLReceiptSP1VerifierV2 sp1Verifier;
        bytes32 sp1VerifierRuntimeCodeHash;
        bytes4 sp1ProofSelector;
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
        bytes32 initialCheckpointCommitment;
        bytes32 initialCommitteeRootCommitment;
        uint64 initialFinalizedHeight;
    }

    struct ReceiptPublicValues {
        uint32 proofProgramVersion;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        bytes32 committeeRootCommitment;
        bytes32 committeeTransitionCommitment;
        bytes32 finalizedBlockCommitment;
        bytes32 finalizedStateRootCommitment;
        uint64 routeEpoch;
        bytes32 policyHashCommitment;
        bytes32 routeIdCommitment;
        bytes32 routeTrustClass;
        bytes32 routeConfigDigestCommitment;
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
        bytes32 proofNullifier;
    }

    struct CheckpointPublicValues {
        uint32 proofProgramVersion;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        bytes32 priorCheckpointCommitment;
        bytes32 resultingCheckpointCommitment;
        uint64 finalizedHeight;
        bytes32 proofNullifier;
        bytes32 committeeRootCommitment;
        bytes32 committeeTransitionCommitment;
    }

    error ZeroAddress(bytes32 field);
    error ZeroValue(bytes32 field);
    error WrongBinding(bytes32 field);
    error InvalidPftlBytes(bytes32 field);
    error ProofTooLarge(uint256 actual, uint256 maximum);
    error PublicValuesTooLarge(uint256 actual, uint256 maximum);
    error NonCanonicalPublicValues();
    error UnknownPriorCheckpoint(bytes32 checkpoint);
    error StaleCheckpoint(uint64 actual, uint64 latest);
    error ProofAlreadyConsumed(bytes32 nullifier);
    error ReceiptAlreadyAccepted(bytes32 receiptCommitment);

    event ReceiptAccepted(
        bytes32 indexed receiptCommitment,
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
    bytes32 public immutable sp1VerifierRuntimeCodeHash;
    bytes4 public immutable sp1ProofSelector;
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

    bytes32 public latestCheckpointCommitment;
    uint64 public latestFinalizedHeight;
    mapping(bytes32 => bool) public acceptedCheckpointCommitment;
    mapping(bytes32 => uint64) public checkpointFinalizedHeight;
    mapping(bytes32 => bytes32) public checkpointCommitteeRootCommitment;
    mapping(bytes32 => bool) public consumedProofNullifier;
    mapping(bytes32 => bool) public acceptedReceiptCommitment;

    constructor(Config memory config) {
        if (address(config.sp1Verifier) == address(0)) revert ZeroAddress("sp1_verifier");
        if (address(config.sp1Verifier).code.length == 0
            || config.sp1VerifierRuntimeCodeHash == bytes32(0)
            || address(config.sp1Verifier).codehash != config.sp1VerifierRuntimeCodeHash) revert WrongBinding("sp1_verifier_code");
        if (config.sp1ProofSelector == bytes4(0)) revert WrongBinding("sp1_proof_selector");
        (bool acceptsInvalid,) = address(config.sp1Verifier).staticcall(
            abi.encodeCall(config.sp1Verifier.verifyProof, (config.programVKey, bytes(""), bytes("")))
        );
        if (acceptsInvalid) revert WrongBinding("sp1_negative_control");
        if (config.controller == address(0)) revert ZeroAddress("controller");
        if (config.wrappedToken == address(0)) revert ZeroAddress("wrapped_token");
        if (
            config.programVKey == bytes32(0) || config.pftlChainIdHash == bytes32(0)
                || config.pftlGenesisHashCommitment == bytes32(0) || config.pftlProtocolVersion == 0
                || config.routeIdCommitment == bytes32(0)
                || config.nativeNavAssetIdCommitment == bytes32(0)
                || config.settlementAssetIdCommitment == bytes32(0)
                || config.destinationChainId == 0 || config.wrappedTokenRuntimeCodeHash == bytes32(0)
                || config.maxProofBytes == 0
                || config.maxPublicValuesBytes == 0 || config.initialCheckpointCommitment == bytes32(0)
                || config.initialCommitteeRootCommitment == bytes32(0)
                || config.initialFinalizedHeight == 0
        ) revert ZeroValue("constructor");
        if (config.destinationChainId != block.chainid) revert WrongBinding("destination_chain_id");
        if (config.wrappedToken.codehash != config.wrappedTokenRuntimeCodeHash) {
            revert WrongBinding("wrapped_token_code_hash");
        }

        sp1Verifier = config.sp1Verifier;
        sp1VerifierRuntimeCodeHash = config.sp1VerifierRuntimeCodeHash;
        sp1ProofSelector = config.sp1ProofSelector;
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
        latestCheckpointCommitment = config.initialCheckpointCommitment;
        latestFinalizedHeight = config.initialFinalizedHeight;
        acceptedCheckpointCommitment[config.initialCheckpointCommitment] = true;
        checkpointFinalizedHeight[config.initialCheckpointCommitment] = config.initialFinalizedHeight;
        checkpointCommitteeRootCommitment[config.initialCheckpointCommitment] = config.initialCommitteeRootCommitment;
    }

    function routeTrustClass() external pure returns (bytes32) {
        return TRUST_CLASS_TRUSTLESS_FINALITY;
    }

    function verifyAndAccept(bytes calldata publicValues, bytes calldata proofBytes)
        external
        returns (bytes32 acceptedCommitment)
    {
        _requireBounds(publicValues, proofBytes);
        ReceiptPublicValues memory decoded = abi.decode(publicValues, (ReceiptPublicValues));
        if (keccak256(publicValues) != keccak256(abi.encode(decoded))) revert NonCanonicalPublicValues();
        _requireReceiptBindings(decoded);
        if (!acceptedCheckpointCommitment[decoded.priorCheckpointCommitment]) {
            revert UnknownPriorCheckpoint(decoded.priorCheckpointCommitment);
        }
        if (consumedProofNullifier[decoded.proofNullifier]) {
            revert ProofAlreadyConsumed(decoded.proofNullifier);
        }
        acceptedCommitment = _receiptCommitment(
            decoded.sourceReceiptRootCommitment,
            decoded.sourceReceiptHashCommitment,
            decoded.routeConfigDigestCommitment,
            decoded.packetDigest
        );
        if (acceptedReceiptCommitment[acceptedCommitment]) {
            revert ReceiptAlreadyAccepted(acceptedCommitment);
        }

        _requireCommitteeProgression(decoded.priorCheckpointCommitment, decoded.committeeRootCommitment, decoded.committeeTransitionCommitment);
        _verifyProof(publicValues, proofBytes);

        consumedProofNullifier[decoded.proofNullifier] = true;
        acceptedReceiptCommitment[acceptedCommitment] = true;
        _recordCheckpoint(
            decoded.priorCheckpointCommitment,
            decoded.resultingCheckpointCommitment,
            decoded.finalizedHeight,
            decoded.committeeRootCommitment
        );
        emit ReceiptAccepted(
            acceptedCommitment,
            decoded.packetDigest,
            decoded.proofNullifier,
            decoded.recipient,
            decoded.mintAmountAtoms,
            decoded.finalizedHeight
        );
    }

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
        if (decoded.priorCheckpointCommitment != latestCheckpointCommitment) {
            revert UnknownPriorCheckpoint(decoded.priorCheckpointCommitment);
        }
        if (decoded.finalizedHeight <= latestFinalizedHeight) {
            revert StaleCheckpoint(decoded.finalizedHeight, latestFinalizedHeight);
        }
        if (consumedProofNullifier[decoded.proofNullifier]) {
            revert ProofAlreadyConsumed(decoded.proofNullifier);
        }
        _requireCommitteeProgression(decoded.priorCheckpointCommitment, decoded.committeeRootCommitment, decoded.committeeTransitionCommitment);
        _verifyProof(publicValues, proofBytes);
        consumedProofNullifier[decoded.proofNullifier] = true;
        _recordCheckpoint(
            decoded.priorCheckpointCommitment,
            decoded.resultingCheckpointCommitment,
            decoded.finalizedHeight,
            decoded.committeeRootCommitment
        );
    }

    function isReceiptAccepted(
        bytes calldata sourceReceiptRoot,
        bytes calldata sourceReceiptHash,
        bytes calldata routeConfigDigest,
        bytes32 assertedRouteTrustClass,
        bytes32 packetDigest
    ) external view returns (bool) {
        if (
            assertedRouteTrustClass != TRUST_CLASS_TRUSTLESS_FINALITY || sourceReceiptRoot.length != 48
                || sourceReceiptHash.length != 48 || routeConfigDigest.length != 48
                || packetDigest == bytes32(0)
        ) return false;
        return acceptedReceiptCommitment[
            _receiptCommitment(
                keccak256(sourceReceiptRoot),
                keccak256(sourceReceiptHash),
                keccak256(routeConfigDigest),
                packetDigest
            )
        ];
    }

    function receiptCommitment(
        bytes calldata sourceReceiptRoot,
        bytes calldata sourceReceiptHash,
        bytes calldata routeConfigDigest,
        bytes32 packetDigest
    ) external pure returns (bytes32) {
        return _receiptCommitment(
            keccak256(sourceReceiptRoot),
            keccak256(sourceReceiptHash),
            keccak256(routeConfigDigest),
            packetDigest
        );
    }

    function _requireReceiptBindings(ReceiptPublicValues memory decoded) private view {
        if (
            decoded.proofProgramVersion != 2 || decoded.pftlChainIdHash != pftlChainIdHash
                || decoded.pftlGenesisHashCommitment != pftlGenesisHashCommitment
                || decoded.pftlProtocolVersion != pftlProtocolVersion
                || decoded.routeEpoch == 0 || decoded.policyHashCommitment == bytes32(0)
                || decoded.routeIdCommitment != routeIdCommitment
                || decoded.nativeNavAssetIdCommitment != nativeNavAssetIdCommitment
                || decoded.settlementAssetIdCommitment != settlementAssetIdCommitment
                || decoded.routeTrustClass != TRUST_CLASS_TRUSTLESS_FINALITY
                || decoded.acceptedReceiptCode != ACCEPTED_EXPORT_RECEIPT_CODE
                || decoded.destinationChainId != destinationChainId || decoded.destinationChainId != block.chainid
                || decoded.controller != controller || decoded.wrappedToken != wrappedToken
                || decoded.packetDigest == bytes32(0) || decoded.recipient == address(0)
                || decoded.mintAmountAtoms == 0 || decoded.settlementValueAtoms == 0
                || decoded.packetNonce == bytes32(0) || decoded.deadline == 0
                || decoded.sourceHeight == 0
                || decoded.resultingCheckpointCommitment == bytes32(0) || decoded.proofNullifier == bytes32(0)
        ) revert WrongBinding("receipt");
        if (
            decoded.routeConfigDigestCommitment == bytes32(0)
                || decoded.committeeRootCommitment == bytes32(0)
                || decoded.finalizedBlockCommitment != decoded.resultingCheckpointCommitment
                || decoded.finalizedStateRootCommitment == bytes32(0)
                || decoded.routeIdCommitment == bytes32(0)
                || decoded.nativeNavAssetIdCommitment == bytes32(0)
                || decoded.settlementAssetIdCommitment == bytes32(0)
                || decoded.pricingNavEpoch == 0
                || decoded.pricingReservePacketHashCommitment == bytes32(0)
                || decoded.sourceWalletCommitment == bytes32(0)
                || decoded.sourceReceiptRootCommitment == bytes32(0)
                || decoded.sourceReceiptHashCommitment == bytes32(0)
        ) revert WrongBinding("receipt_commitments");
        if (controller.code.length == 0) revert WrongBinding("controller_code");
        if (wrappedToken.codehash != wrappedTokenRuntimeCodeHash) revert WrongBinding("wrapped_token_code_hash");
    }

    // The guest proves every intervening transition. The on-chain anchor is
    // independently provisioned at deployment, never selected by the witness.
    function _requireCommitteeProgression(bytes32 prior, bytes32 resultingRoot, bytes32 transitionStart) private view {
        bytes32 trustedRoot = checkpointCommitteeRootCommitment[prior];
        if (trustedRoot == bytes32(0)) revert UnknownPriorCheckpoint(prior);
        if (resultingRoot == bytes32(0)) revert WrongBinding("committee_root");
        if (transitionStart == bytes32(0)) {
            if (resultingRoot != trustedRoot) revert WrongBinding("committee_root");
        } else if (transitionStart != trustedRoot) {
            revert WrongBinding("committee_transition_start");
        }
    }

    function _recordCheckpoint(bytes32 prior, bytes32 resulting, uint64 finalizedHeight, bytes32 committeeRoot) private {
        if (resulting == bytes32(0)) revert ZeroValue("resulting_checkpoint");
        if (acceptedCheckpointCommitment[resulting]) {
            if (checkpointCommitteeRootCommitment[resulting] != committeeRoot) revert WrongBinding("recorded_committee_root");
            if (checkpointFinalizedHeight[resulting] != finalizedHeight) revert WrongBinding("recorded_checkpoint_height");
            return;
        }
        if (prior != latestCheckpointCommitment) revert UnknownPriorCheckpoint(prior);
        if (finalizedHeight <= latestFinalizedHeight) revert StaleCheckpoint(finalizedHeight, latestFinalizedHeight);
        acceptedCheckpointCommitment[resulting] = true;
        checkpointFinalizedHeight[resulting] = finalizedHeight;
        checkpointCommitteeRootCommitment[resulting] = committeeRoot;
        if (finalizedHeight > latestFinalizedHeight) {
            latestCheckpointCommitment = resulting;
            latestFinalizedHeight = finalizedHeight;
            emit CheckpointAdvanced(prior, resulting, finalizedHeight);
        }
    }

    function _verifyProof(bytes calldata publicValues, bytes calldata proofBytes) private view {
        if (proofBytes.length < 4 || bytes4(proofBytes[:4]) != sp1ProofSelector) revert WrongBinding("sp1_proof_selector");
        if (address(sp1Verifier).code.length == 0 || address(sp1Verifier).codehash != sp1VerifierRuntimeCodeHash) {
            revert WrongBinding("sp1_verifier_code");
        }
        sp1Verifier.verifyProof(programVKey, publicValues, proofBytes);
    }

    function _requireBounds(bytes calldata publicValues, bytes calldata proofBytes) private view {
        if (proofBytes.length == 0 || proofBytes.length > maxProofBytes) {
            revert ProofTooLarge(proofBytes.length, maxProofBytes);
        }
        if (publicValues.length == 0 || publicValues.length > maxPublicValuesBytes) {
            revert PublicValuesTooLarge(publicValues.length, maxPublicValuesBytes);
        }
    }

    function _receiptCommitment(
        bytes32 sourceReceiptRootCommitment,
        bytes32 sourceReceiptHashCommitment,
        bytes32 routeConfigDigestCommitment,
        bytes32 packetDigest
    ) private pure returns (bytes32) {
        return keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.accepted_receipt.v1",
                sourceReceiptRootCommitment,
                sourceReceiptHashCommitment,
                routeConfigDigestCommitment,
                TRUST_CLASS_TRUSTLESS_FINALITY,
                packetDigest
            )
        );
    }
}
