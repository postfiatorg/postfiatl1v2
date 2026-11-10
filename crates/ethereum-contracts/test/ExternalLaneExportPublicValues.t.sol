// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTLFinalityVerifierV1, ISP1Verifier} from "../src/PFTLFinalityVerifierV1.sol";
import {ExternalLaneExportFixture as F} from "./fixtures/ExternalLaneExportFixture.sol";

contract LaneFixtureSP1 is ISP1Verifier {
    function verifyProof(bytes32, bytes calldata, bytes calldata) external pure {}
}

contract LaneFixtureToken {}

/// Decodes the public values the Rust side builds for one `external_lane_export_v1`
/// packet through the real verifier, configured with the lane's derived route.
contract ExternalLaneExportPublicValuesTest {
    PFTLFinalityVerifierV1 private verifier;

    function setUp() public {
        LaneFixtureToken token = new LaneFixtureToken();
        verifier = new PFTLFinalityVerifierV1(
            PFTLFinalityVerifierV1.Config({
                sp1Verifier: new LaneFixtureSP1(),
                programVKey: keccak256("lane-program-vkey"),
                pftlChainIdHash: keccak256(bytes(F.PFTL_CHAIN_ID)),
                pftlGenesisHashCommitment: keccak256(F.PFTL_GENESIS_HASH),
                pftlProtocolVersion: F.PFTL_PROTOCOL_VERSION,
                routeProfileHashCommitment: keccak256(F.ROUTE_PROFILE_HASH),
                routeEpoch: F.ROUTE_EPOCH,
                assetIdCommitment: keccak256(F.ASSET_ID),
                arbitrumChainId: F.SOURCE_CHAIN_ID,
                vaultRuntimeCodeHash: F.LOCK_CODE_HASH,
                token: address(token),
                tokenRuntimeCodeHash: address(token).codehash,
                maxProofBytes: 4096,
                maxPublicValuesBytes: 16384,
                initialCheckpointCommitment: keccak256(F.PRIOR_CHECKPOINT),
                initialFinalizedHeight: 10,
                initialCommitteeRootCommitment: keccak256(F.COMMITTEE_ROOT)
            })
        );
    }

    function testLaneExportPublicValuesDecodeToEveryField() public view {
        PFTLFinalityVerifierV1.DecodedEgress memory d = verifier.decodePublicValues(F.PUBLIC_VALUES);
        require(d.proofProgramVersion == 1, "proof_program_version");
        require(d.pftlChainIdHash == verifier.pftlChainIdHash(), "pftl_chain_id");
        require(d.pftlGenesisHashCommitment == verifier.pftlGenesisHashCommitment(), "pftl_genesis_hash");
        require(d.pftlProtocolVersion == verifier.pftlProtocolVersion(), "protocol_version");
        require(d.routeProfileHashCommitment == verifier.routeProfileHashCommitment(), "route_profile_hash");
        require(d.routeEpoch == verifier.routeEpoch(), "route_epoch");
        require(d.priorCheckpointCommitment == verifier.latestCheckpointCommitment(), "prior_checkpoint");
        require(d.resultingCheckpointCommitment == keccak256(F.RESULTING_CHECKPOINT), "resulting_checkpoint");
        require(d.committeeRootCommitment == verifier.latestCommitteeRootCommitment(), "committee_root");
        require(d.committeeTransitionCommitment == bytes32(0), "committee_transition");
        require(d.finalizedBlockHeight == F.FINALIZED_HEIGHT, "finalized_height");
        require(d.bridgeExitRootCommitment == keccak256(F.EXPORT_HISTORY_ROOT), "export_history_root");
        require(d.acceptedReceiptCommitment == keccak256(F.TX_ID), "accepted_receipt");
        require(d.assetIdCommitment == verifier.assetIdCommitment(), "asset_id");
        require(d.burnTxIdCommitment == keccak256(F.BURN_TX_ID), "burn_tx_id");
        require(d.withdrawalIdCommitment == keccak256(F.WITHDRAWAL_ID), "withdrawal_id");
        require(d.amount == F.AMOUNT, "amount");
        require(d.recipient == address(F.RECIPIENT), "recipient");
        require(d.withdrawalFinalizedHeight == F.EXPORTED_AT_HEIGHT, "withdrawal_height");
        require(d.withdrawalFinalizedHeight <= d.finalizedBlockHeight, "withdrawal_height_order");
        require(d.arbitrumChainId == verifier.arbitrumChainId(), "chain_id");
        require(d.vault == address(F.LOCK), "vault");
        require(d.vaultRuntimeCodeHash == verifier.vaultRuntimeCodeHash(), "vault_code_hash");
        require(d.token == address(F.TOKEN), "token");
        require(d.tokenRuntimeCodeHash == F.TOKEN_CODE_HASH, "token_code_hash");
        require(d.packetDigest == F.PACKET_EVM_DIGEST, "packet_digest");
        require(d.withdrawalPacketHashCommitment == keccak256(F.PACKET_DIGEST), "packet_hash");
        require(d.proofNullifier == F.PROOF_NULLIFIER, "proof_nullifier");
    }

    function testLaneExportPublicValuesRefuseAnyTrailingByte() public {
        (bool ok,) = address(verifier)
            .call(abi.encodeCall(PFTLFinalityVerifierV1.decodePublicValues, (bytes.concat(F.PUBLIC_VALUES, hex"00"))));
        require(!ok, "trailing byte must not decode");
    }
}
