// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    IPFTLReceiptSP1VerifierV2,
    PFTLReceiptFinalityVerifierV2
} from "../src/PFTLReceiptFinalityVerifierV2.sol";

interface VmReceiptV2 {
    function expectRevert(bytes calldata revertData) external;
}

contract MockSP1ReceiptVerifierV2 is IPFTLReceiptSP1VerifierV2 {
    function verifyProof(bytes32, bytes calldata, bytes calldata proofBytes) external pure {
        require(keccak256(proofBytes) == keccak256(hex"0102"), "bad mock proof");
    }
}

contract ReceiptBoundTokenV2 {}
contract ReceiptBoundControllerV2 {}

/// Adversarial tests for the P0-01 remedies. The three forgeries named in the
/// locked spec are tested explicitly, and the three concerns are tested apart
/// from each other so a pass on one cannot stand in for another.
contract PFTLReceiptFinalityVerifierV2Test {
    VmReceiptV2 internal constant vm =
        VmReceiptV2(address(uint160(uint256(keccak256("hevm cheat code")))));

    MockSP1ReceiptVerifierV2 internal sp1;
    ReceiptBoundTokenV2 internal token;
    ReceiptBoundControllerV2 internal controller;
    PFTLReceiptFinalityVerifierV2 internal verifier;

    bytes32 internal constant ANCHOR = bytes32(uint256(100));
    uint64 internal constant ANCHOR_HEIGHT = 50;
    bytes32 internal constant PROVEN_COMMITTEE = bytes32(uint256(777));
    bytes32 internal constant SEGMENT = bytes32(uint256(888));

    function setUp() public {
        sp1 = new MockSP1ReceiptVerifierV2();
        token = new ReceiptBoundTokenV2();
        controller = new ReceiptBoundControllerV2();
        verifier = new PFTLReceiptFinalityVerifierV2(
            PFTLReceiptFinalityVerifierV2.Config({
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
                maxPublicValuesBytes: 8192,
                ancestryAnchorCommitment: ANCHOR,
                ancestryAnchorHeight: ANCHOR_HEIGHT
            })
        );
    }

    // ----------------------------------------------------------- happy path

    function testAcceptsReceiptWithProvenAncestryAndAdvancesTip() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        bytes32 id = verifier.verifyAndAccept(abi.encode(v), hex"0102");
        require(id == v.receiptIdentifier, "wrong identifier returned");
        require(verifier.consumedReceiptIdentifier(id), "receipt not consumed");
        require(verifier.pinnedTipHeight() == 60, "tip not advanced");
        require(verifier.pinnedTipCommitment() == v.resultingCheckpointCommitment, "tip wrong");
    }

    // ------------------------------------------------------- (1) ANCESTRY

    /// P0-01 forgery 1: a made-up committee.
    function testRejectsMadeUpCommittee() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        v.committeeRootCommitment = bytes32(uint256(0xdead));
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.CommitteeRootNotProven.selector,
                bytes32(uint256(0xdead)),
                PROVEN_COMMITTEE
            )
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    /// A committee root supplied outside the proven segment is rejected even when
    /// it is nonzero -- the exact check V1 relied on.
    function testRejectsNonzeroCommitteeThatIsNotTheProvenRoot() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        v.committeeRootCommitment = bytes32(uint256(1));
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.CommitteeRootNotProven.selector,
                bytes32(uint256(1)),
                PROVEN_COMMITTEE
            )
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    /// A proof rooted anywhere other than the pinned anchor is rejected.
    function testRejectsProofRootedElsewhere() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        v.ancestryRootCommitment = bytes32(uint256(0xbeef));
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.WrongAncestryRoot.selector,
                bytes32(uint256(0xbeef)),
                ANCHOR
            )
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    function testRejectsAbsentAncestrySegment() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        v.ancestrySegmentCommitment = bytes32(0);
        vm.expectRevert(
            abi.encodeWithSelector(PFTLReceiptFinalityVerifierV2.AbsentAncestrySegment.selector)
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    /// P0-01 forgery 2: genuine prior checkpoint, made-up checkpoint below the
    /// tip. Under V1 the made-up checkpoint was recorded as accepted and became
    /// a usable `prior`. Here the made-up prior is simply not canonical.
    function testRejectsGenuinePriorWithMadeUpCheckpointBelowTip() public {
        // Advance the tip legitimately to 60.
        verifier.verifyAndAccept(abi.encode(_values()), hex"0102");
        require(verifier.pinnedTipHeight() == 60, "precondition");

        // Claim a checkpoint below the tip, carried on a fresh receipt.
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory low = _values();
        low.finalizedHeight = 55;
        low.resultingCheckpointCommitment = bytes32(uint256(0xfa11));
        low.receiptIdentifier = bytes32(uint256(0xfa12));
        low.proofNullifier = bytes32(uint256(0xfa13));
        verifier.verifyAndAccept(abi.encode(low), hex"0102");

        // It must NOT have become canonical, so it cannot be used as a prior.
        require(verifier.canonicalCheckpointHeight(bytes32(uint256(0xfa11))) == 0,
            "below-tip checkpoint became canonical");
        require(verifier.pinnedTipHeight() == 60, "tip moved backwards");

        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory forged = _values();
        forged.priorCheckpointCommitment = bytes32(uint256(0xfa11));
        forged.receiptIdentifier = bytes32(uint256(0xfa14));
        forged.proofNullifier = bytes32(uint256(0xfa15));
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.UnknownPriorCheckpoint.selector,
                bytes32(uint256(0xfa11))
            )
        );
        verifier.verifyAndAccept(abi.encode(forged), hex"0102");
    }

    // ------------------------------------------------------- (2) ORDERING

    function testTipAdvancesOnlyStrictlyAbove() public {
        PFTLReceiptFinalityVerifierV2.CheckpointPublicValues memory c = _checkpoint(60);
        verifier.advanceCheckpoint(abi.encode(c), hex"0102");
        require(verifier.pinnedTipHeight() == 60, "did not advance");

        // Equal height cannot advance.
        PFTLReceiptFinalityVerifierV2.CheckpointPublicValues memory same = _checkpoint(60);
        same.priorCheckpointCommitment = verifier.pinnedTipCommitment();
        same.proofNullifier = bytes32(uint256(0xee01));
        vm.expectRevert(
            abi.encodeWithSelector(PFTLReceiptFinalityVerifierV2.StaleCheckpoint.selector, 60, 60)
        );
        verifier.advanceCheckpoint(abi.encode(same), hex"0102");

        // Below cannot advance.
        PFTLReceiptFinalityVerifierV2.CheckpointPublicValues memory below = _checkpoint(59);
        below.priorCheckpointCommitment = verifier.pinnedTipCommitment();
        below.proofNullifier = bytes32(uint256(0xee02));
        vm.expectRevert(
            abi.encodeWithSelector(PFTLReceiptFinalityVerifierV2.StaleCheckpoint.selector, 59, 60)
        );
        verifier.advanceCheckpoint(abi.encode(below), hex"0102");

        // A rejection leaves the stored tip untouched.
        require(verifier.pinnedTipHeight() == 60, "tip changed on rejection");
    }

    /// Ordering is not ancestry: a HIGHER checkpoint with bad ancestry is rejected.
    function testRejectsHigherCheckpointWithInvalidAncestry() public {
        PFTLReceiptFinalityVerifierV2.CheckpointPublicValues memory c = _checkpoint(999);
        c.ancestryRootCommitment = bytes32(uint256(0xbad));
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.WrongAncestryRoot.selector,
                bytes32(uint256(0xbad)),
                ANCHOR
            )
        );
        verifier.advanceCheckpoint(abi.encode(c), hex"0102");
        require(verifier.pinnedTipHeight() == ANCHOR_HEIGHT, "tip changed on rejection");
    }

    // --------------------------------------------------------- (3) REPLAY

    function testRejectsImmediateReceiptResubmission() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.ReceiptAlreadyConsumed.selector, v.receiptIdentifier
            )
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    /// P0-01 forgery 3: the same source receipt re-proved against a different,
    /// perfectly valid later checkpoint. Replay prevention must not depend on the
    /// checkpoint, which is why the identifier is route-scoped and
    /// checkpoint-independent.
    function testRejectsSameReceiptReProvedAgainstLaterCheckpoint() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory first = _values();
        verifier.verifyAndAccept(abi.encode(first), hex"0102");

        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory again = _values();
        again.finalizedHeight = 70;
        again.resultingCheckpointCommitment = bytes32(uint256(0xc0de));
        again.priorCheckpointCommitment = first.resultingCheckpointCommitment;
        again.proofNullifier = bytes32(uint256(0xc0df)); // a genuinely new proof
        // Same receipt identifier: the same underlying export.
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.ReceiptAlreadyConsumed.selector, again.receiptIdentifier
            )
        );
        verifier.verifyAndAccept(abi.encode(again), hex"0102");
    }

    function testRejectsReplayedProofNullifier() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        verifier.verifyAndAccept(abi.encode(v), hex"0102");

        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory other = _values();
        other.receiptIdentifier = bytes32(uint256(0xaa01)); // different receipt
        other.priorCheckpointCommitment = v.resultingCheckpointCommitment;
        other.finalizedHeight = 70;
        other.resultingCheckpointCommitment = bytes32(uint256(0xaa02));
        // Same nullifier as the accepted proof.
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.ProofAlreadyConsumed.selector, other.proofNullifier
            )
        );
        verifier.verifyAndAccept(abi.encode(other), hex"0102");
    }

    // ------------------------------------------------- binding / fee policy

    function testRejectsMissingMultiplierPolicyCommitment() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        v.multiplierPolicyCommitment = bytes32(0);
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.WrongBinding.selector, bytes32("receipt_commitments")
            )
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    function testRejectsWrongRouteIdentityBeforeSP1() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        v.routeIdCommitment = bytes32(uint256(999));
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.WrongBinding.selector, bytes32("receipt")
            )
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    function testRejectsV1ProgramVersion() public {
        PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v = _values();
        v.proofProgramVersion = 1;
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLReceiptFinalityVerifierV2.WrongBinding.selector, bytes32("receipt")
            )
        );
        verifier.verifyAndAccept(abi.encode(v), hex"0102");
    }

    // --------------------------------------------------------------- fixtures

    function _values()
        internal
        view
        returns (PFTLReceiptFinalityVerifierV2.ReceiptPublicValues memory v)
    {
        v = PFTLReceiptFinalityVerifierV2.ReceiptPublicValues({
            proofProgramVersion: 2,
            pftlChainIdHash: bytes32(uint256(2)),
            pftlGenesisHashCommitment: bytes32(uint256(3)),
            pftlProtocolVersion: 1,
            ancestryRootCommitment: ANCHOR,
            ancestrySegmentCommitment: SEGMENT,
            provenCommitteeRootCommitment: PROVEN_COMMITTEE,
            committeeRootCommitment: PROVEN_COMMITTEE,
            committeeTransitionCommitment: bytes32(uint256(5)),
            finalizedBlockCommitment: bytes32(uint256(6)),
            finalizedStateRootCommitment: bytes32(uint256(7)),
            routeEpoch: 2,
            policyHashCommitment: bytes32(uint256(8)),
            routeIdCommitment: bytes32(uint256(35)),
            routeTrustClass: verifier.TRUST_CLASS_TRUSTLESS_FINALITY(),
            routeConfigDigestCommitment: bytes32(uint256(9)),
            multiplierPolicyCommitment: bytes32(uint256(10)),
            nativeNavAssetIdCommitment: bytes32(uint256(36)),
            settlementAssetIdCommitment: bytes32(uint256(37)),
            pricingNavEpoch: 1,
            pricingReservePacketHashCommitment: bytes32(uint256(11)),
            sourceWalletCommitment: bytes32(uint256(12)),
            sourceReceiptRootCommitment: bytes32(uint256(13)),
            sourceReceiptHashCommitment: bytes32(uint256(14)),
            acceptedReceiptCode: verifier.ACCEPTED_EXPORT_RECEIPT_CODE(),
            packetDigest: bytes32(uint256(15)),
            destinationChainId: block.chainid,
            controller: address(controller),
            wrappedToken: address(token),
            recipient: address(uint160(uint256(16))),
            mintAmountAtoms: 1_000_000,
            settlementValueAtoms: 1_000_000,
            packetNonce: bytes32(uint256(17)),
            deadline: 1,
            sourceHeight: 59,
            priorCheckpointCommitment: ANCHOR,
            resultingCheckpointCommitment: bytes32(uint256(101)),
            finalizedHeight: 60,
            receiptIdentifier: bytes32(uint256(18)),
            proofNullifier: bytes32(uint256(19))
        });
    }

    function _checkpoint(uint64 height)
        internal
        view
        returns (PFTLReceiptFinalityVerifierV2.CheckpointPublicValues memory c)
    {
        c = PFTLReceiptFinalityVerifierV2.CheckpointPublicValues({
            proofProgramVersion: 2,
            pftlChainIdHash: bytes32(uint256(2)),
            pftlGenesisHashCommitment: bytes32(uint256(3)),
            pftlProtocolVersion: 1,
            ancestryRootCommitment: ANCHOR,
            ancestrySegmentCommitment: SEGMENT,
            provenCommitteeRootCommitment: PROVEN_COMMITTEE,
            committeeRootCommitment: PROVEN_COMMITTEE,
            priorCheckpointCommitment: ANCHOR,
            resultingCheckpointCommitment: bytes32(uint256(uint256(height) + 1000)),
            finalizedHeight: height,
            proofNullifier: bytes32(uint256(uint256(height) + 5000))
        });
    }
}
