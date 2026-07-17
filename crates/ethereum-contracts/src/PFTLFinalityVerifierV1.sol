// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @notice The stable SP1 verifier/gateway ABI published by Succinct.
interface ISP1Verifier {
    function verifyProof(bytes32 programVKey, bytes calldata publicValues, bytes calldata proofBytes) external view;
}

/// @notice Stateful verifier for proof-native pfUSDC withdrawals.
/// @dev The SP1 program proves PFTL consensus-v2 finality, an accepted receipt,
///      and membership in the voted bridge-exit root. This contract pins the
///      chain/route/contracts, advances a proof-verified checkpoint, and
///      consumes each proof nullifier and withdrawal exactly once.
contract PFTLFinalityVerifierV1 {
    struct Config {
        ISP1Verifier sp1Verifier;
        bytes32 programVKey;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        bytes32 routeProfileHashCommitment;
        uint64 routeEpoch;
        bytes32 assetIdCommitment;
        uint64 arbitrumChainId;
        bytes32 vaultRuntimeCodeHash;
        address token;
        bytes32 tokenRuntimeCodeHash;
        uint256 maxProofBytes;
        uint256 maxPublicValuesBytes;
        bytes32 initialCheckpointCommitment;
        uint64 initialFinalizedHeight;
        bytes32 initialCommitteeRootCommitment;
    }

    struct DecodedEgress {
        uint32 proofProgramVersion;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        bytes32 routeProfileHashCommitment;
        uint64 routeEpoch;
        bytes32 priorCheckpointCommitment;
        bytes32 resultingCheckpointCommitment;
        bytes32 committeeRootCommitment;
        bytes32 committeeTransitionCommitment;
        uint64 finalizedBlockHeight;
        bytes32 bridgeExitRootCommitment;
        bytes32 acceptedReceiptCommitment;
        bytes32 assetIdCommitment;
        bytes32 burnTxIdCommitment;
        bytes32 withdrawalIdCommitment;
        uint256 amount;
        address recipient;
        uint64 withdrawalFinalizedHeight;
        uint64 arbitrumChainId;
        address vault;
        bytes32 vaultRuntimeCodeHash;
        address token;
        bytes32 tokenRuntimeCodeHash;
        bytes32 packetDigest;
        bytes32 withdrawalPacketHashCommitment;
        bytes32 proofNullifier;
    }

    struct DecodedCheckpoint {
        uint32 proofProgramVersion;
        bytes32 pftlChainIdHash;
        bytes32 pftlGenesisHashCommitment;
        uint32 pftlProtocolVersion;
        bytes32 priorCheckpointCommitment;
        bytes32 resultingCheckpointCommitment;
        bytes32 committeeRootCommitment;
        bytes32 committeeTransitionCommitment;
        uint64 finalizedBlockHeight;
    }

    error ZeroAddress(bytes32 field);
    error ZeroValue(bytes32 field);
    error ProofTooLarge(uint256 actual, uint256 maximum);
    error PublicValuesTooLarge(uint256 actual, uint256 maximum);
    error NonCanonicalPublicValues(bytes32 field);
    error WrongBinding(bytes32 field);
    error StaleCheckpoint(uint64 actual, uint64 latest);
    error UnknownPriorCheckpoint(bytes32 checkpoint);
    error ProofAlreadyConsumed(bytes32 nullifier);
    error WithdrawalAlreadyConsumed(bytes32 withdrawalIdCommitment);

    event PFTLCheckpointAdvanced(
        bytes32 indexed priorCheckpointCommitment,
        bytes32 indexed resultingCheckpointCommitment,
        uint64 finalizedBlockHeight,
        bytes32 committeeRootCommitment
    );
    event PFTLHistoricalCheckpointRecognized(
        bytes32 indexed priorCheckpointCommitment,
        bytes32 indexed resultingCheckpointCommitment,
        uint64 finalizedBlockHeight,
        bytes32 committeeRootCommitment
    );
    event PFTLWithdrawalVerified(
        bytes32 indexed proofNullifier,
        bytes32 indexed withdrawalIdCommitment,
        bytes32 indexed burnTxIdCommitment,
        address recipient,
        uint256 amount
    );

    bytes private constant CANONICAL_MAGIC = "PFTL-PFUSDC-TIER4";
    bytes private constant EGRESS_SCHEMA = "postfiat.pfusdc.egress_public_values.v1";
    bytes private constant CHECKPOINT_SCHEMA = "postfiat.pfusdc.checkpoint_public_values.v1";
    bytes32 private constant ACCEPTED_CODE_HASH = keccak256("accepted");

    ISP1Verifier public immutable sp1Verifier;
    bytes32 public immutable programVKey;
    bytes32 public immutable pftlChainIdHash;
    bytes32 public immutable pftlGenesisHashCommitment;
    uint32 public immutable pftlProtocolVersion;
    bytes32 public immutable routeProfileHashCommitment;
    uint64 public immutable routeEpoch;
    bytes32 public immutable assetIdCommitment;
    uint64 public immutable arbitrumChainId;
    bytes32 public immutable vaultRuntimeCodeHash;
    address public immutable token;
    bytes32 public immutable tokenRuntimeCodeHash;
    uint256 public immutable maxProofBytes;
    uint256 public immutable maxPublicValuesBytes;

    uint64 public latestFinalizedHeight;
    bytes32 public latestCheckpointCommitment;
    bytes32 public latestCommitteeRootCommitment;
    mapping(bytes32 => bool) public acceptedCheckpointCommitment;
    mapping(bytes32 => bytes32) public checkpointCommitteeRootCommitment;
    mapping(bytes32 => bool) public consumedProofNullifier;
    mapping(bytes32 => bool) public consumedWithdrawalIdCommitment;

    constructor(Config memory config) {
        if (address(config.sp1Verifier) == address(0)) revert ZeroAddress("sp1_verifier");
        if (config.token == address(0)) revert ZeroAddress("token");
        if (
            config.programVKey == bytes32(0) || config.pftlChainIdHash == bytes32(0)
                || config.pftlGenesisHashCommitment == bytes32(0) || config.pftlProtocolVersion == 0
                || config.routeProfileHashCommitment == bytes32(0) || config.routeEpoch == 0
                || config.assetIdCommitment == bytes32(0) || config.arbitrumChainId == 0
                || config.vaultRuntimeCodeHash == bytes32(0) || config.tokenRuntimeCodeHash == bytes32(0)
                || config.maxProofBytes == 0 || config.maxPublicValuesBytes == 0
                || config.initialCheckpointCommitment == bytes32(0) || config.initialFinalizedHeight == 0
                || config.initialCommitteeRootCommitment == bytes32(0)
        ) revert ZeroValue("constructor");
        if (config.token.codehash != config.tokenRuntimeCodeHash) revert WrongBinding("token_code_hash");

        sp1Verifier = config.sp1Verifier;
        programVKey = config.programVKey;
        pftlChainIdHash = config.pftlChainIdHash;
        pftlGenesisHashCommitment = config.pftlGenesisHashCommitment;
        pftlProtocolVersion = config.pftlProtocolVersion;
        routeProfileHashCommitment = config.routeProfileHashCommitment;
        routeEpoch = config.routeEpoch;
        assetIdCommitment = config.assetIdCommitment;
        arbitrumChainId = config.arbitrumChainId;
        vaultRuntimeCodeHash = config.vaultRuntimeCodeHash;
        token = config.token;
        tokenRuntimeCodeHash = config.tokenRuntimeCodeHash;
        maxProofBytes = config.maxProofBytes;
        maxPublicValuesBytes = config.maxPublicValuesBytes;
        latestCheckpointCommitment = config.initialCheckpointCommitment;
        latestFinalizedHeight = config.initialFinalizedHeight;
        latestCommitteeRootCommitment = config.initialCommitteeRootCommitment;
        acceptedCheckpointCommitment[config.initialCheckpointCommitment] = true;
        checkpointCommitteeRootCommitment[config.initialCheckpointCommitment] =
            config.initialCommitteeRootCommitment;
    }

    function verifyAndConsume(bytes calldata publicValues, bytes calldata proofBytes)
        external
        returns (
            address recipient,
            uint256 amount,
            bytes32 withdrawalIdCommitment,
            bytes32 burnTxIdCommitment,
            bytes32 packetDigest
        )
    {
        if (proofBytes.length == 0 || proofBytes.length > maxProofBytes) {
            revert ProofTooLarge(proofBytes.length, maxProofBytes);
        }
        if (publicValues.length == 0 || publicValues.length > maxPublicValuesBytes) {
            revert PublicValuesTooLarge(publicValues.length, maxPublicValuesBytes);
        }
        DecodedEgress memory decoded = _decode(publicValues);
        _requireBindings(decoded);
        if (!acceptedCheckpointCommitment[decoded.priorCheckpointCommitment]) {
            revert UnknownPriorCheckpoint(decoded.priorCheckpointCommitment);
        }
        if (consumedProofNullifier[decoded.proofNullifier]) {
            revert ProofAlreadyConsumed(decoded.proofNullifier);
        }
        if (consumedWithdrawalIdCommitment[decoded.withdrawalIdCommitment]) {
            revert WithdrawalAlreadyConsumed(decoded.withdrawalIdCommitment);
        }

        sp1Verifier.verifyProof(programVKey, publicValues, proofBytes);

        consumedProofNullifier[decoded.proofNullifier] = true;
        consumedWithdrawalIdCommitment[decoded.withdrawalIdCommitment] = true;
        _recordCheckpoint(
            decoded.priorCheckpointCommitment,
            decoded.resultingCheckpointCommitment,
            decoded.finalizedBlockHeight,
            decoded.committeeRootCommitment
        );
        emit PFTLWithdrawalVerified(
            decoded.proofNullifier,
            decoded.withdrawalIdCommitment,
            decoded.burnTxIdCommitment,
            decoded.recipient,
            decoded.amount
        );
        return (
            decoded.recipient,
            decoded.amount,
            decoded.withdrawalIdCommitment,
            decoded.burnTxIdCommitment,
            decoded.packetDigest
        );
    }

    /// @notice Advances the bounded PFTL light-client checkpoint without
    /// authorizing a withdrawal. This keeps the next withdrawal proof bounded
    /// even during long periods with no bridge exits.
    function advanceCheckpoint(bytes calldata publicValues, bytes calldata proofBytes) external {
        if (proofBytes.length == 0 || proofBytes.length > maxProofBytes) {
            revert ProofTooLarge(proofBytes.length, maxProofBytes);
        }
        if (publicValues.length == 0 || publicValues.length > maxPublicValuesBytes) {
            revert PublicValuesTooLarge(publicValues.length, maxPublicValuesBytes);
        }
        DecodedCheckpoint memory decoded = _decodeCheckpoint(publicValues);
        _requireCheckpointBindings(decoded);
        if (decoded.priorCheckpointCommitment != latestCheckpointCommitment) {
            revert UnknownPriorCheckpoint(decoded.priorCheckpointCommitment);
        }
        if (decoded.finalizedBlockHeight <= latestFinalizedHeight) {
            revert StaleCheckpoint(decoded.finalizedBlockHeight, latestFinalizedHeight);
        }
        sp1Verifier.verifyProof(programVKey, publicValues, proofBytes);
        _recordCheckpoint(
            decoded.priorCheckpointCommitment,
            decoded.resultingCheckpointCommitment,
            decoded.finalizedBlockHeight,
            decoded.committeeRootCommitment
        );
    }

    function decodePublicValues(bytes calldata publicValues) external pure returns (DecodedEgress memory) {
        return _decode(publicValues);
    }

    function _requireBindings(DecodedEgress memory decoded) private view {
        if (decoded.proofProgramVersion != 1) revert WrongBinding("proof_program_version");
        if (decoded.pftlChainIdHash != pftlChainIdHash) revert WrongBinding("pftl_chain_id");
        if (decoded.pftlGenesisHashCommitment != pftlGenesisHashCommitment) {
            revert WrongBinding("pftl_genesis_hash");
        }
        if (decoded.pftlProtocolVersion != pftlProtocolVersion) revert WrongBinding("protocol_version");
        if (decoded.routeProfileHashCommitment != routeProfileHashCommitment) {
            revert WrongBinding("route_profile_hash");
        }
        if (decoded.routeEpoch != routeEpoch) revert WrongBinding("route_epoch");
        _requireCommitteeProgression(
            decoded.priorCheckpointCommitment,
            decoded.committeeRootCommitment,
            decoded.committeeTransitionCommitment
        );
        if (decoded.assetIdCommitment != assetIdCommitment) revert WrongBinding("asset_id");
        if (decoded.arbitrumChainId != arbitrumChainId || decoded.arbitrumChainId != block.chainid) {
            revert WrongBinding("arbitrum_chain_id");
        }
        if (decoded.vault != msg.sender || msg.sender.codehash != vaultRuntimeCodeHash) {
            revert WrongBinding("vault");
        }
        if (decoded.vaultRuntimeCodeHash != vaultRuntimeCodeHash) revert WrongBinding("vault_code_hash");
        if (decoded.token != token || decoded.tokenRuntimeCodeHash != tokenRuntimeCodeHash) {
            revert WrongBinding("token");
        }
        if (token.codehash != tokenRuntimeCodeHash) revert WrongBinding("live_token_code_hash");
        if (decoded.amount == 0 || decoded.recipient == address(0)) revert WrongBinding("payout");
        if (decoded.withdrawalFinalizedHeight == 0 || decoded.withdrawalFinalizedHeight > decoded.finalizedBlockHeight) {
            revert WrongBinding("withdrawal_height");
        }
    }

    function _requireCheckpointBindings(DecodedCheckpoint memory decoded) private view {
        if (decoded.proofProgramVersion != 1) revert WrongBinding("proof_program_version");
        if (decoded.pftlChainIdHash != pftlChainIdHash) revert WrongBinding("pftl_chain_id");
        if (decoded.pftlGenesisHashCommitment != pftlGenesisHashCommitment) {
            revert WrongBinding("pftl_genesis_hash");
        }
        if (decoded.pftlProtocolVersion != pftlProtocolVersion) revert WrongBinding("protocol_version");
        _requireCommitteeProgression(
            decoded.priorCheckpointCommitment,
            decoded.committeeRootCommitment,
            decoded.committeeTransitionCommitment
        );
    }

    function _requireCommitteeProgression(
        bytes32 priorCheckpointCommitment,
        bytes32 resultingCommitteeRootCommitment,
        bytes32 transitionStartCommitment
    ) private view {
        bytes32 priorCommitteeRoot = checkpointCommitteeRootCommitment[priorCheckpointCommitment];
        if (priorCommitteeRoot == bytes32(0)) revert UnknownPriorCheckpoint(priorCheckpointCommitment);
        if (transitionStartCommitment == bytes32(0)) {
            if (resultingCommitteeRootCommitment != priorCommitteeRoot) {
                revert WrongBinding("committee_root");
            }
        } else if (transitionStartCommitment != priorCommitteeRoot) {
            revert WrongBinding("committee_transition_start");
        }
    }

    function _recordCheckpoint(
        bytes32 priorCheckpointCommitment,
        bytes32 resultingCheckpointCommitment,
        uint64 finalizedBlockHeight,
        bytes32 committeeRootCommitment
    ) private {
        bytes32 recordedCommittee = checkpointCommitteeRootCommitment[resultingCheckpointCommitment];
        if (recordedCommittee != bytes32(0) && recordedCommittee != committeeRootCommitment) {
            revert WrongBinding("recorded_committee_root");
        }
        acceptedCheckpointCommitment[resultingCheckpointCommitment] = true;
        checkpointCommitteeRootCommitment[resultingCheckpointCommitment] = committeeRootCommitment;
        if (finalizedBlockHeight > latestFinalizedHeight) {
            latestCheckpointCommitment = resultingCheckpointCommitment;
            latestFinalizedHeight = finalizedBlockHeight;
            latestCommitteeRootCommitment = committeeRootCommitment;
            emit PFTLCheckpointAdvanced(
                priorCheckpointCommitment,
                resultingCheckpointCommitment,
                finalizedBlockHeight,
                committeeRootCommitment
            );
        } else {
            emit PFTLHistoricalCheckpointRecognized(
                priorCheckpointCommitment,
                resultingCheckpointCommitment,
                finalizedBlockHeight,
                committeeRootCommitment
            );
        }
    }

    function _decode(bytes calldata data) private pure returns (DecodedEgress memory out) {
        uint256 cursor;
        if (data.length < CANONICAL_MAGIC.length + 4) revert NonCanonicalPublicValues("prefix");
        if (keccak256(data[0:CANONICAL_MAGIC.length]) != keccak256(CANONICAL_MAGIC)) {
            revert NonCanonicalPublicValues("magic");
        }
        cursor = CANONICAL_MAGIC.length;
        uint256 schemaLength = _u32(data, cursor);
        cursor += 4;
        if (schemaLength != EGRESS_SCHEMA.length || cursor + schemaLength > data.length) {
            revert NonCanonicalPublicValues("schema_prefix");
        }
        if (keccak256(data[cursor:cursor + schemaLength]) != keccak256(EGRESS_SCHEMA)) {
            revert NonCanonicalPublicValues("schema_prefix");
        }
        cursor += schemaLength;

        uint256 start;
        (start, cursor) = _field(data, cursor, 1, EGRESS_SCHEMA.length);
        if (keccak256(data[start:start + EGRESS_SCHEMA.length]) != keccak256(EGRESS_SCHEMA)) {
            revert NonCanonicalPublicValues("schema");
        }
        (start, cursor) = _field(data, cursor, 2, 4);
        out.proofProgramVersion = _u32(data, start);
        (start, cursor) = _fieldVariable(data, cursor, 3, 1, 256);
        out.pftlChainIdHash = keccak256(data[start:cursor]);
        (start, cursor) = _field(data, cursor, 4, 48);
        out.pftlGenesisHashCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 5, 4);
        out.pftlProtocolVersion = _u32(data, start);
        (start, cursor) = _field(data, cursor, 6, 48);
        out.routeProfileHashCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 7, 8);
        out.routeEpoch = _u64(data, start);
        (start, cursor) = _field(data, cursor, 8, 48);
        out.priorCheckpointCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 9, 48);
        out.resultingCheckpointCommitment = keccak256(data[start:start + 48]);
        (, cursor) = _field(data, cursor, 10, 8);
        (start, cursor) = _field(data, cursor, 11, 48);
        out.committeeRootCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _fieldVariable(data, cursor, 12, 0, 48);
        if (cursor - start != 0 && cursor - start != 48) revert NonCanonicalPublicValues("committee_transition");
        if (cursor - start == 48) {
            out.committeeTransitionCommitment = keccak256(data[start:start + 48]);
        }
        (start, cursor) = _field(data, cursor, 13, 8);
        out.finalizedBlockHeight = _u64(data, start);
        (, cursor) = _field(data, cursor, 14, 8);
        (, cursor) = _field(data, cursor, 15, 48);
        (, cursor) = _field(data, cursor, 16, 48);
        (, cursor) = _field(data, cursor, 17, 48);
        (start, cursor) = _field(data, cursor, 18, 48);
        out.bridgeExitRootCommitment = keccak256(data[start:start + 48]);
        (, cursor) = _field(data, cursor, 19, 8);
        (, cursor) = _field(data, cursor, 20, 48);
        (start, cursor) = _field(data, cursor, 21, 48);
        out.acceptedReceiptCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 22, 8);
        if (keccak256(data[start:start + 8]) != ACCEPTED_CODE_HASH) revert WrongBinding("receipt_code");
        (start, cursor) = _field(data, cursor, 23, 48);
        out.assetIdCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 24, 48);
        out.burnTxIdCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 25, 48);
        out.withdrawalIdCommitment = keccak256(data[start:start + 48]);
        (, cursor) = _field(data, cursor, 26, 48);
        (start, cursor) = _field(data, cursor, 27, 8);
        out.amount = _u64(data, start);
        (start, cursor) = _field(data, cursor, 28, 20);
        out.recipient = _address(data, start);
        (, cursor) = _field(data, cursor, 29, 48);
        (, cursor) = _field(data, cursor, 30, 48);
        (start, cursor) = _field(data, cursor, 31, 8);
        out.withdrawalFinalizedHeight = _u64(data, start);
        (start, cursor) = _field(data, cursor, 32, 8);
        out.arbitrumChainId = _u64(data, start);
        (start, cursor) = _field(data, cursor, 33, 20);
        out.vault = _address(data, start);
        (start, cursor) = _field(data, cursor, 34, 32);
        out.vaultRuntimeCodeHash = _bytes32(data, start);
        (start, cursor) = _field(data, cursor, 35, 20);
        out.token = _address(data, start);
        (start, cursor) = _field(data, cursor, 36, 32);
        out.tokenRuntimeCodeHash = _bytes32(data, start);
        (start, cursor) = _field(data, cursor, 37, 32);
        out.packetDigest = _bytes32(data, start);
        (start, cursor) = _field(data, cursor, 38, 48);
        out.withdrawalPacketHashCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 39, 32);
        out.proofNullifier = _bytes32(data, start);
        if (cursor != data.length) revert NonCanonicalPublicValues("trailing_bytes");
    }

    function _decodeCheckpoint(bytes calldata data) private pure returns (DecodedCheckpoint memory out) {
        uint256 cursor;
        if (data.length < CANONICAL_MAGIC.length + 4) revert NonCanonicalPublicValues("prefix");
        if (keccak256(data[0:CANONICAL_MAGIC.length]) != keccak256(CANONICAL_MAGIC)) {
            revert NonCanonicalPublicValues("magic");
        }
        cursor = CANONICAL_MAGIC.length;
        uint256 schemaLength = _u32(data, cursor);
        cursor += 4;
        if (schemaLength != CHECKPOINT_SCHEMA.length || cursor + schemaLength > data.length) {
            revert NonCanonicalPublicValues("schema_prefix");
        }
        if (keccak256(data[cursor:cursor + schemaLength]) != keccak256(CHECKPOINT_SCHEMA)) {
            revert NonCanonicalPublicValues("schema_prefix");
        }
        cursor += schemaLength;

        uint256 start;
        (start, cursor) = _field(data, cursor, 1, CHECKPOINT_SCHEMA.length);
        if (keccak256(data[start:start + CHECKPOINT_SCHEMA.length]) != keccak256(CHECKPOINT_SCHEMA)) {
            revert NonCanonicalPublicValues("schema");
        }
        (start, cursor) = _field(data, cursor, 2, 4);
        out.proofProgramVersion = _u32(data, start);
        (start, cursor) = _fieldVariable(data, cursor, 3, 1, 256);
        out.pftlChainIdHash = keccak256(data[start:cursor]);
        (start, cursor) = _field(data, cursor, 4, 48);
        out.pftlGenesisHashCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 5, 4);
        out.pftlProtocolVersion = _u32(data, start);
        (start, cursor) = _field(data, cursor, 6, 48);
        out.priorCheckpointCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _field(data, cursor, 7, 48);
        out.resultingCheckpointCommitment = keccak256(data[start:start + 48]);
        (, cursor) = _field(data, cursor, 8, 8);
        (start, cursor) = _field(data, cursor, 9, 48);
        out.committeeRootCommitment = keccak256(data[start:start + 48]);
        (start, cursor) = _fieldVariable(data, cursor, 10, 0, 48);
        if (cursor - start != 0 && cursor - start != 48) revert NonCanonicalPublicValues("committee_transition");
        if (cursor - start == 48) {
            out.committeeTransitionCommitment = keccak256(data[start:start + 48]);
        }
        (start, cursor) = _field(data, cursor, 11, 8);
        out.finalizedBlockHeight = _u64(data, start);
        (, cursor) = _field(data, cursor, 12, 8);
        (, cursor) = _field(data, cursor, 13, 48);
        (, cursor) = _field(data, cursor, 14, 48);
        (, cursor) = _field(data, cursor, 15, 48);
        if (cursor != data.length) revert NonCanonicalPublicValues("trailing_bytes");
    }

    function _field(bytes calldata data, uint256 cursor, uint16 expectedTag, uint256 expectedLength)
        private
        pure
        returns (uint256 start, uint256 next)
    {
        (start, next) = _fieldVariable(data, cursor, expectedTag, expectedLength, expectedLength);
    }

    function _fieldVariable(
        bytes calldata data,
        uint256 cursor,
        uint16 expectedTag,
        uint256 minimumLength,
        uint256 maximumLength
    ) private pure returns (uint256 start, uint256 next) {
        if (cursor + 6 > data.length) revert NonCanonicalPublicValues("field_header");
        uint16 tag;
        uint32 length;
        assembly {
            tag := shr(240, calldataload(add(data.offset, cursor)))
            length := shr(224, calldataload(add(add(data.offset, cursor), 2)))
        }
        if (tag != expectedTag) revert NonCanonicalPublicValues("field_tag");
        if (length < minimumLength || length > maximumLength) revert NonCanonicalPublicValues("field_length");
        start = cursor + 6;
        next = start + uint256(length);
        if (next > data.length) revert NonCanonicalPublicValues("field_bounds");
    }

    function _u32(bytes calldata data, uint256 offset) private pure returns (uint32 value) {
        if (offset + 4 > data.length) revert NonCanonicalPublicValues("u32");
        assembly {
            value := shr(224, calldataload(add(data.offset, offset)))
        }
    }

    function _u64(bytes calldata data, uint256 offset) private pure returns (uint64 value) {
        if (offset + 8 > data.length) revert NonCanonicalPublicValues("u64");
        assembly {
            value := shr(192, calldataload(add(data.offset, offset)))
        }
    }

    function _bytes32(bytes calldata data, uint256 offset) private pure returns (bytes32 value) {
        if (offset + 32 > data.length) revert NonCanonicalPublicValues("bytes32");
        assembly {
            value := calldataload(add(data.offset, offset))
        }
    }

    function _address(bytes calldata data, uint256 offset) private pure returns (address value) {
        if (offset + 20 > data.length) revert NonCanonicalPublicValues("address");
        assembly {
            value := shr(96, calldataload(add(data.offset, offset)))
        }
    }
}
