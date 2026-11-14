// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;
import {ReceiptV2MintFixture} from "./ReceiptV2MintProof.t.sol";
import {PFTLReceiptFinalityVerifierV2} from "../src/PFTLReceiptFinalityVerifierV2.sol";
import {PFTLUniswapPrimaryMarketV2} from "../src/PFTLUniswapPrimaryMarketV2.sol";

contract ReceiptV2RetiredProofTest is ReceiptV2MintFixture {
    function testRetiredCommitteeValidProofCannotMintFromStaleBranch() public {
        bytes memory canonical = vm.readFileBinary("canonical-checkpoint/public-values.bin");
        bytes memory canonicalProof = vm.readFileBinary("canonical-checkpoint/proof-calldata.bin");
        PFTLReceiptFinalityVerifierV2.CheckpointPublicValues memory checkpoint =
            abi.decode(canonical, (PFTLReceiptFinalityVerifierV2.CheckpointPublicValues));
        require(checkpoint.pftlChainIdHash == receiptValues.pftlChainIdHash
            && checkpoint.pftlGenesisHashCommitment == receiptValues.pftlGenesisHashCommitment
            && checkpoint.pftlProtocolVersion == receiptValues.pftlProtocolVersion
            && checkpoint.priorCheckpointCommitment == receiptValues.priorCheckpointCommitment,
            "checkpoint and receipt are from different chains/ancestors");
        require(checkpoint.committeeTransitionCommitment == receiptValues.committeeRootCommitment
            && checkpoint.committeeRootCommitment != receiptValues.committeeRootCommitment
            && checkpoint.finalizedHeight > receiptValues.finalizedHeight, "committee was not retired");
        // Both are cryptographically valid statements. Only the rotated
        // checkpoint extends the destination's selected canonical branch.
        sp1.verifyProof(key, canonical, canonicalProof);
        sp1.verifyProof(key, publicValues, proof);
        verifier.advanceCheckpoint(canonical, canonicalProof);
        require(verifier.latestCheckpointCommitment() == checkpoint.resultingCheckpointCommitment
            && verifier.latestFinalizedHeight() == checkpoint.finalizedHeight,
            "canonical checkpoint did not advance");
        vm.expectRevert(abi.encodeWithSelector(PFTLReceiptFinalityVerifierV2.UnknownPriorCheckpoint.selector,
            receiptValues.priorCheckpointCommitment));
        accept(publicValues, proof);
        require(verifier.latestCheckpointCommitment() == checkpoint.resultingCheckpointCommitment
            && verifier.latestFinalizedHeight() == checkpoint.finalizedHeight, "stale proof moved tip");
        require(verifier.checkpointCommitteeRootCommitment(checkpoint.resultingCheckpointCommitment)
            == checkpoint.committeeRootCommitment, "canonical committee changed");
        require(!verifier.consumedProofNullifier(receiptValues.proofNullifier)
            && !verifier.acceptedCheckpointCommitment(receiptValues.resultingCheckpointCommitment),
            "stale proof mutated replay/ancestry state");
        require(!verifier.isReceiptAccepted(packet.sourceReceiptRoot, packet.sourceReceiptHash,
            packet.routeConfigDigest, verifier.TRUST_CLASS_TRUSTLESS_FINALITY(), receiptValues.packetDigest),
            "stale receipt was accepted");
        market.setMintPaused(false);
        vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.ReceiptNotAccepted.selector,
            receiptValues.packetDigest));
        market.consumeMintOnly(packet);
        noMint();
    }
}
