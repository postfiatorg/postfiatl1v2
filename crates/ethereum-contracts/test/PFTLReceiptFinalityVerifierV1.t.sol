// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    IPFTLReceiptSP1Verifier,
    PFTLReceiptFinalityVerifierV1
} from "../src/PFTLReceiptFinalityVerifierV1.sol";

interface VmReceiptV1 {
    function expectRevert(bytes calldata revertData) external;
}

contract MockSP1ReceiptVerifier is IPFTLReceiptSP1Verifier {
    function verifyProof(bytes32, bytes calldata, bytes calldata proofBytes) external pure {
        require(keccak256(proofBytes) == keccak256(hex"0102"), "bad mock proof");
    }
}

contract ReceiptBoundToken {}
contract ReceiptBoundController {}

contract PFTLReceiptFinalityVerifierV1Test {
    VmReceiptV1 internal constant vm =
        VmReceiptV1(address(uint160(uint256(keccak256("hevm cheat code")))));

    MockSP1ReceiptVerifier internal sp1;
    ReceiptBoundToken internal token;
    ReceiptBoundController internal controller;
    PFTLReceiptFinalityVerifierV1 internal verifier;

    bytes32 internal constant INITIAL_CHECKPOINT = bytes32(uint256(100));

    function setUp() public {
        sp1 = new MockSP1ReceiptVerifier();
        token = new ReceiptBoundToken();
        controller = new ReceiptBoundController();
        verifier = new PFTLReceiptFinalityVerifierV1(
            PFTLReceiptFinalityVerifierV1.Config({
                sp1Verifier: sp1,
                programVKey: bytes32(uint256(1)),
                pftlChainIdHash: bytes32(uint256(2)),
                pftlGenesisHashCommitment: bytes32(uint256(3)),
                pftlProtocolVersion: 1,
                routeIdCommitment: bytes32(uint256(35)),
                nativeNavAssetIdCommitment: bytes32(uint256(36)),
                settlementAssetIdCommitment: bytes32(uint256(37)),
                destinationChainId: block.chainid,
                controller: address(controller),
                wrappedToken: address(token),
                wrappedTokenRuntimeCodeHash: address(token).codehash,
                maxProofBytes: 1024,
                maxPublicValuesBytes: 4096,
                initialCheckpointCommitment: INITIAL_CHECKPOINT,
                initialFinalizedHeight: 50
            })
        );
    }

    function testProofAcceptsExactReceiptAndRejectsNullifierReplay() public {
        PFTLReceiptFinalityVerifierV1.ReceiptPublicValues memory values = _values();
        bytes memory encoded = abi.encode(values);
        bytes32 receipt = verifier.verifyAndAccept(encoded, hex"0102");
        require(verifier.acceptedReceiptCommitment(receipt), "receipt not accepted");
        require(verifier.latestFinalizedHeight() == 60, "height not advanced");

        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV1.ProofAlreadyConsumed.selector,
                values.proofNullifier
            )
        );
        verifier.verifyAndAccept(encoded, hex"0102");
    }

    function testWrongRouteIdentityFailsBeforeSP1() public {
        PFTLReceiptFinalityVerifierV1.ReceiptPublicValues memory values = _values();
        values.routeIdCommitment = bytes32(uint256(999));
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV1.WrongBinding.selector,
                bytes32("receipt")
            )
        );
        verifier.verifyAndAccept(abi.encode(values), hex"0102");
    }

    function testTrailingPublicValuesFailCanonicalEncoding() public {
        bytes memory encoded = bytes.concat(abi.encode(_values()), hex"00");
        vm.expectRevert(
            abi.encodeWithSelector(PFTLReceiptFinalityVerifierV1.NonCanonicalPublicValues.selector)
        );
        verifier.verifyAndAccept(encoded, hex"0102");
    }

    function testCheckpointProofAdvancesTheAnchorForLaterReceiptProof() public {
        PFTLReceiptFinalityVerifierV1.CheckpointPublicValues memory checkpoint =
            PFTLReceiptFinalityVerifierV1.CheckpointPublicValues({
                proofProgramVersion: 1,
                pftlChainIdHash: bytes32(uint256(2)),
                pftlGenesisHashCommitment: bytes32(uint256(3)),
                pftlProtocolVersion: 1,
                priorCheckpointCommitment: INITIAL_CHECKPOINT,
                resultingCheckpointCommitment: bytes32(uint256(101)),
                finalizedHeight: 60,
                proofNullifier: bytes32(uint256(201))
            });
        verifier.advanceCheckpoint(abi.encode(checkpoint), hex"0102");

        PFTLReceiptFinalityVerifierV1.ReceiptPublicValues memory values = _values();
        values.priorCheckpointCommitment = checkpoint.resultingCheckpointCommitment;
        values.resultingCheckpointCommitment = bytes32(uint256(102));
        values.finalizedHeight = 70;
        values.proofNullifier = bytes32(uint256(202));
        bytes32 receipt = verifier.verifyAndAccept(abi.encode(values), hex"0102");
        require(verifier.acceptedReceiptCommitment(receipt), "receipt not accepted");
        require(verifier.latestFinalizedHeight() == 70, "height not advanced");
    }

    function _values()
        private
        view
        returns (PFTLReceiptFinalityVerifierV1.ReceiptPublicValues memory values)
    {
        values = PFTLReceiptFinalityVerifierV1.ReceiptPublicValues({
            proofProgramVersion: 1,
            pftlChainIdHash: bytes32(uint256(2)),
            pftlGenesisHashCommitment: bytes32(uint256(3)),
            pftlProtocolVersion: 1,
            committeeRootCommitment: bytes32(uint256(31)),
            committeeTransitionCommitment: bytes32(uint256(32)),
            finalizedBlockCommitment: bytes32(uint256(33)),
            finalizedStateRootCommitment: bytes32(uint256(34)),
            routeEpoch: 7,
            policyHashCommitment: bytes32(uint256(4)),
            routeIdCommitment: bytes32(uint256(35)),
            routeTrustClass: keccak256("TRUSTLESS_FINALITY"),
            routeConfigDigestCommitment: keccak256(
                bytes.concat(bytes32(uint256(5)), bytes16(uint128(6)))
            ),
            nativeNavAssetIdCommitment: bytes32(uint256(36)),
            settlementAssetIdCommitment: bytes32(uint256(37)),
            pricingNavEpoch: 8,
            pricingReservePacketHashCommitment: bytes32(uint256(38)),
            sourceWalletCommitment: bytes32(uint256(39)),
            sourceReceiptRootCommitment: keccak256(
                bytes.concat(bytes32(uint256(7)), bytes16(uint128(8)))
            ),
            sourceReceiptHashCommitment: keccak256(
                bytes.concat(bytes32(uint256(9)), bytes16(uint128(10)))
            ),
            acceptedReceiptCode: keccak256("export_debit"),
            packetDigest: bytes32(uint256(11)),
            destinationChainId: block.chainid,
            controller: address(controller),
            wrappedToken: address(token),
            recipient: address(0xA666),
            mintAmountAtoms: 250_000e6,
            settlementValueAtoms: 251_250e6,
            packetNonce: bytes32(uint256(40)),
            deadline: 1_800_000_000,
            sourceHeight: 55,
            priorCheckpointCommitment: INITIAL_CHECKPOINT,
            resultingCheckpointCommitment: bytes32(uint256(101)),
            finalizedHeight: 60,
            proofNullifier: bytes32(uint256(102))
        });
    }
}
