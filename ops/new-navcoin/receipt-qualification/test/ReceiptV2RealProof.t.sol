// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;
interface IRealSP1Verifier {
    function verifyProof(bytes32, bytes calldata, bytes calldata) external view;
    function VERIFIER_HASH() external pure returns (bytes32);
}
import {IPFTLReceiptSP1VerifierV2, PFTLReceiptFinalityVerifierV2} from "../src/PFTLReceiptFinalityVerifierV2.sol";
interface VmRealReceipt {
    function getCode(string calldata) external view returns (bytes memory);
    function readFileBinary(string calldata) external view returns (bytes memory);
    function envBytes32(string calldata) external view returns (bytes32);
    function expectRevert() external;
    function expectRevert(bytes calldata) external;
    function etch(address, bytes calldata) external;
    function chainId(uint256) external;
    function prank(address) external;
    function warp(uint256) external;
}
contract FixtureToken {}
contract FixtureController {}
contract ReceiptV2ProofFixture {
    VmRealReceipt constant vm = VmRealReceipt(address(uint160(uint256(keccak256("hevm cheat code")))));
    IRealSP1Verifier sp1;
    FixtureToken token;
    FixtureController controller;
    PFTLReceiptFinalityVerifierV2 verifier;
    PFTLReceiptFinalityVerifierV2.CheckpointPublicValues values;
    PFTLReceiptFinalityVerifierV2.ReceiptPublicValues receiptValues;
    bool isReceipt;
    bytes publicValues;
    bytes proof;
    bytes32 key;
    function setUp() public virtual {
        publicValues = vm.readFileBinary("public-values.bin");
        proof = vm.readFileBinary("proof-calldata.bin");
        isReceipt = publicValues.length == 1120;
        if (isReceipt) {
            receiptValues = abi.decode(publicValues, (PFTLReceiptFinalityVerifierV2.ReceiptPublicValues));
            values = PFTLReceiptFinalityVerifierV2.CheckpointPublicValues({
                proofProgramVersion: receiptValues.proofProgramVersion,
                pftlChainIdHash: receiptValues.pftlChainIdHash,
                pftlGenesisHashCommitment: receiptValues.pftlGenesisHashCommitment,
                pftlProtocolVersion: receiptValues.pftlProtocolVersion,
                priorCheckpointCommitment: receiptValues.priorCheckpointCommitment,
                resultingCheckpointCommitment: receiptValues.resultingCheckpointCommitment,
                finalizedHeight: receiptValues.finalizedHeight, proofNullifier: receiptValues.proofNullifier,
                committeeRootCommitment: receiptValues.committeeRootCommitment,
                committeeTransitionCommitment: receiptValues.committeeTransitionCommitment
            });
        } else { values = abi.decode(publicValues, (PFTLReceiptFinalityVerifierV2.CheckpointPublicValues)); }
        key = vm.envBytes32("PFTL_V2_PROGRAM_KEY");
        require(values.proofProgramVersion == 2 && values.finalizedHeight > 1, "wrong fixture");
        // Actual Ethereum SP1 runtime inherited by the local Anvil fork.
        // The input receipt is an archived SYNTHETIC-COMMITTEE fixture.
        address deployed = address(bytes20(hex"397a5f7f3dbd538f23de225b51f532c34448da9b"));
        require(deployed.code.length != 0, "mainnet SP1 runtime missing from fork");
        sp1 = IRealSP1Verifier(deployed);
        token = new FixtureToken();
        controller = new FixtureController();
        if (isReceipt) {
            vm.chainId(receiptValues.destinationChainId);
            vm.etch(receiptValues.wrappedToken, address(token).code);
            vm.etch(receiptValues.controller, address(controller).code);
            token = FixtureToken(receiptValues.wrappedToken);
            controller = FixtureController(receiptValues.controller);
        }
        verifier = new PFTLReceiptFinalityVerifierV2(config());
    }
    function config() internal view returns (PFTLReceiptFinalityVerifierV2.Config memory) {
        return PFTLReceiptFinalityVerifierV2.Config({
            sp1Verifier: IPFTLReceiptSP1VerifierV2(address(sp1)),
            sp1VerifierRuntimeCodeHash: address(sp1).codehash,
            sp1ProofSelector: bytes4(proof), programVKey: key,
            pftlChainIdHash: values.pftlChainIdHash,
            pftlGenesisHashCommitment: values.pftlGenesisHashCommitment,
            pftlProtocolVersion: values.pftlProtocolVersion,
            routeIdCommitment: isReceipt ? receiptValues.routeIdCommitment : bytes32(uint256(1)),
            nativeNavAssetIdCommitment: isReceipt ? receiptValues.nativeNavAssetIdCommitment : bytes32(uint256(2)),
            settlementAssetIdCommitment: isReceipt ? receiptValues.settlementAssetIdCommitment : bytes32(uint256(3)),
            destinationChainId: block.chainid,
            controller: address(controller), wrappedToken: address(token),
            wrappedTokenRuntimeCodeHash: address(token).codehash,
            maxProofBytes: 4096, maxPublicValuesBytes: 4096,
            initialCheckpointCommitment: values.priorCheckpointCommitment,
            initialCommitteeRootCommitment: values.committeeTransitionCommitment == bytes32(0)
                ? values.committeeRootCommitment : values.committeeTransitionCommitment,
            initialFinalizedHeight: values.finalizedHeight - 1
        });
    }
    function accept(bytes memory encoded, bytes memory proofBytes) internal returns (bytes32) {
        if (isReceipt) {
            return verifier.verifyAndAccept(encoded, proofBytes);
        } else { verifier.advanceCheckpoint(encoded, proofBytes); }
        return bytes32(0);
    }
    function unchanged() internal view {
        require(verifier.latestCheckpointCommitment() == values.priorCheckpointCommitment, "tip changed");
        require(verifier.latestFinalizedHeight() == values.finalizedHeight - 1, "height changed");
        require(!verifier.consumedProofNullifier(values.proofNullifier), "nullifier consumed");
        require(!verifier.acceptedCheckpointCommitment(values.resultingCheckpointCommitment), "checkpoint accepted");
    }
}
contract ReceiptV2RealProofTest is ReceiptV2ProofFixture {
    function testGenuineProofAdvancesAuthenticatedCommitteeAndRejectsReplay() public {
        sp1.verifyProof(key, publicValues, proof);
        bytes32 accepted = accept(publicValues, proof);
        if (isReceipt) { require(verifier.acceptedReceiptCommitment(accepted), "receipt not recorded"); }
        require(verifier.latestCheckpointCommitment() == values.resultingCheckpointCommitment, "wrong tip");
        require(verifier.checkpointCommitteeRootCommitment(values.resultingCheckpointCommitment)
            == values.committeeRootCommitment, "wrong committee");
        require(verifier.consumedProofNullifier(values.proofNullifier), "missing nullifier");
        if (isReceipt) {
            vm.expectRevert(abi.encodeWithSelector(PFTLReceiptFinalityVerifierV2.ProofAlreadyConsumed.selector,
                values.proofNullifier));
        } else {
            vm.expectRevert(abi.encodeWithSelector(PFTLReceiptFinalityVerifierV2.UnknownPriorCheckpoint.selector,
                values.priorCheckpointCommitment));
        }
        accept(publicValues, proof);
    }
    function testCorruptedGenuineProofRejectsWithoutMutation() public {
        proof[proof.length - 1] = bytes1(uint8(proof[proof.length - 1]) ^ 1);
        vm.expectRevert(); accept(publicValues, proof); unchanged();
    }
    function testAlteredPublicValuesRejectWithoutMutation() public {
        bytes memory altered;
        if (isReceipt) {
            PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory bad = receiptValues;
            bad.mintAmountAtoms += 1; altered = abi.encode(bad);
        } else {
            PFTLReceiptFinalityVerifierV2.CheckpointPublicValues memory bad = values;
            bad.resultingCheckpointCommitment ^= bytes32(uint256(1)); altered = abi.encode(bad);
        }
        vm.expectRevert(); accept(altered, proof); unchanged();
    }
    function testWrongProgramKeyRejectsGenuineProof() public {
        PFTLReceiptFinalityVerifierV2.Config memory c = config(); c.programVKey ^= bytes32(uint256(1));
        verifier = new PFTLReceiptFinalityVerifierV2(c);
        vm.expectRevert(); accept(publicValues, proof); unchanged();
    }
    function testWrongSelectorRejectsWithoutMutation() public {
        proof[0] = bytes1(uint8(proof[0]) ^ 1);
        vm.expectRevert(); accept(publicValues, proof); unchanged();
    }
    function testChangedVerifierRuntimeRejectsWithoutMutation() public {
        vm.etch(address(sp1), hex"00");
        vm.expectRevert(); accept(publicValues, proof); unchanged();
    }
    function testUntrustedCommitteeCannotUseGenuineProof() public {
        PFTLReceiptFinalityVerifierV2.Config memory c = config();
        c.initialCommitteeRootCommitment ^= bytes32(uint256(1));
        verifier = new PFTLReceiptFinalityVerifierV2(c);
        vm.expectRevert(); accept(publicValues, proof); unchanged();
    }
    function testHistoricalSiblingCannotUseGenuineProof() public {
        PFTLReceiptFinalityVerifierV2.Config memory c = config();
        c.initialCheckpointCommitment ^= bytes32(uint256(1));
        verifier = new PFTLReceiptFinalityVerifierV2(c);
        vm.expectRevert(); accept(publicValues, proof);
        require(verifier.latestCheckpointCommitment() == c.initialCheckpointCommitment, "stale branch changed tip");
        require(!verifier.consumedProofNullifier(values.proofNullifier), "stale branch consumed nullifier");
    }
}
