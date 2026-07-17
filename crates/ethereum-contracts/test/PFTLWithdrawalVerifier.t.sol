// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTLWithdrawalVerifier} from "../src/PFTLWithdrawalVerifier.sol";

interface WithdrawalVerifierVm {
    function addr(uint256 privateKey) external returns (address);
    function sign(uint256 privateKey, bytes32 digest) external returns (uint8 v, bytes32 r, bytes32 s);
    function warp(uint256 timestamp) external;
    function prank(address sender) external;
}

contract PFTLWithdrawalVerifierTest {
    WithdrawalVerifierVm private constant vm =
        WithdrawalVerifierVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant SIGNER_ONE_KEY = 0xA11CE;
    uint256 private constant SIGNER_TWO_KEY = 0xB0B;
    uint256 private constant SIGNER_THREE_KEY = 0xCAFE;
    uint64 private constant CHALLENGE_DELAY = 100;
    uint64 private constant EXECUTION_WINDOW = 1_000;
    bytes32 private constant PACKET_DIGEST = 0x13bc06baf6052bc02bfbf35f2b5474424c76f90dc5d30e8fb6fb46c08463f2bf;
    bytes32 private constant PFTL_HASH_COMMITMENT = 0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee;
    uint64 private constant PFTL_FINALIZED_HEIGHT = 77;

    PFTLWithdrawalVerifier private verifier;

    function setUp() public {
        address[] memory signers = new address[](3);
        signers[0] = vm.addr(SIGNER_ONE_KEY);
        signers[1] = vm.addr(SIGNER_TWO_KEY);
        signers[2] = vm.addr(SIGNER_THREE_KEY);
        verifier = new PFTLWithdrawalVerifier(address(this), signers, 2, CHALLENGE_DELAY, EXECUTION_WINDOW);
    }

    function testThresholdProofFinalizesAccepted() public {
        vm.warp(1_000);
        bytes32 digest = verifier.proofDigest(PACKET_DIGEST, PFTL_HASH_COMMITMENT, PFTL_FINALIZED_HEIGHT);
        bytes[] memory signatures = _sortedTwoSignatures(SIGNER_ONE_KEY, SIGNER_TWO_KEY, digest);

        bytes32 pending_id =
            verifier.submitProof(PACKET_DIGEST, PFTL_HASH_COMMITMENT, PFTL_FINALIZED_HEIGHT, signatures);
        _assertEq(
            uint256(verifier.getProofStatus(pending_id)), uint256(PFTLWithdrawalVerifier.ProofStatus.Pending), "pending"
        );
        _assertTrue(!verifier.isWithdrawalAccepted(PACKET_DIGEST, PFTL_HASH_COMMITMENT), "not accepted before delay");
        _expectFinalizeRevert(pending_id);

        vm.warp(1_100);
        verifier.finalizeProof(pending_id);
        _assertEq(
            uint256(verifier.getProofStatus(pending_id)),
            uint256(PFTLWithdrawalVerifier.ProofStatus.Accepted),
            "accepted"
        );
        _assertTrue(verifier.isWithdrawalAccepted(PACKET_DIGEST, PFTL_HASH_COMMITMENT), "accepted");
    }

    function testChallengedProofFreezesAndNeverAuthorizes() public {
        vm.warp(1_000);
        bytes32 pending_id = _submitValidProof();

        verifier.challengeProof(pending_id, PFTLWithdrawalVerifier.ChallengeFault.WrongPFTLHash);
        _assertEq(
            uint256(verifier.getProofStatus(pending_id)),
            uint256(PFTLWithdrawalVerifier.ProofStatus.Challenged),
            "challenged"
        );

        vm.warp(1_100);
        verifier.finalizeProof(pending_id);
        _assertEq(
            uint256(verifier.getProofStatus(pending_id)), uint256(PFTLWithdrawalVerifier.ProofStatus.Frozen), "frozen"
        );
        _assertTrue(!verifier.isWithdrawalAccepted(PACKET_DIGEST, PFTL_HASH_COMMITMENT), "frozen not accepted");
    }

    function testUnauthorizedChallengeCannotFreezeValidProof() public {
        vm.warp(1_000);
        bytes32 pending_id = _submitValidProof();

        vm.prank(address(0xBAD));
        _expectChallengeRevert(pending_id);

        vm.warp(1_100);
        verifier.finalizeProof(pending_id);
        _assertEq(
            uint256(verifier.getProofStatus(pending_id)),
            uint256(PFTLWithdrawalVerifier.ProofStatus.Accepted),
            "accepted"
        );
        _assertTrue(verifier.isWithdrawalAccepted(PACKET_DIGEST, PFTL_HASH_COMMITMENT), "accepted after grief attempt");
    }

    function testBadSignatureSetsRejected() public {
        bytes32 digest = verifier.proofDigest(PACKET_DIGEST, PFTL_HASH_COMMITMENT, PFTL_FINALIZED_HEIGHT);
        bytes[] memory one_signature = new bytes[](1);
        one_signature[0] = _signature(SIGNER_ONE_KEY, digest);
        _expectSubmitRevert(one_signature);

        bytes[] memory duplicate_signature = new bytes[](2);
        duplicate_signature[0] = _signature(SIGNER_ONE_KEY, digest);
        duplicate_signature[1] = _signature(SIGNER_ONE_KEY, digest);
        _expectSubmitRevert(duplicate_signature);

        bytes[] memory bad_signer = _sortedTwoSignatures(SIGNER_ONE_KEY, 0xD00D, digest);
        _expectSubmitRevert(bad_signer);
    }

    function testAcceptedProofExpires() public {
        vm.warp(1_000);
        bytes32 pending_id = _submitValidProof();
        vm.warp(1_100);
        verifier.finalizeProof(pending_id);
        _assertTrue(verifier.isWithdrawalAccepted(PACKET_DIGEST, PFTL_HASH_COMMITMENT), "accepted initially");

        vm.warp(2_101);
        _assertTrue(!verifier.isWithdrawalAccepted(PACKET_DIGEST, PFTL_HASH_COMMITMENT), "expired");
    }

    function testWithdrawalPlanIdsMatchPFTLVector() public view {
        bytes32 packet_digest = 0xd3a51b4b26388c6b5a65a95cfccd3b3c8118b22bc18b8e0ec7f8f7fc5c130b84;
        bytes32 hash_commitment = 0xc04691b95e3006772b9bafe7cddbcfbaf7fb9585fb61afb1d84102be2e169e22;

        _assertEq(
            verifier.pendingProofId(packet_digest, hash_commitment, 14),
            0x7b2601b02906efe417bc99dc7c70302550e5d66091492d6a0ab2f2f22c965403,
            "pending proof id"
        );
        _assertEq(
            verifier.withdrawalKey(packet_digest, hash_commitment),
            0x4bdbed3b21db9620f30d416aedde9e195bf82c30be5206ef044766ace730fcb6,
            "withdrawal key"
        );
    }

    function _submitValidProof() private returns (bytes32 pending_id) {
        bytes32 digest = verifier.proofDigest(PACKET_DIGEST, PFTL_HASH_COMMITMENT, PFTL_FINALIZED_HEIGHT);
        bytes[] memory signatures = _sortedTwoSignatures(SIGNER_ONE_KEY, SIGNER_TWO_KEY, digest);
        pending_id = verifier.submitProof(PACKET_DIGEST, PFTL_HASH_COMMITMENT, PFTL_FINALIZED_HEIGHT, signatures);
    }

    function _sortedTwoSignatures(uint256 left_key, uint256 right_key, bytes32 digest)
        private
        returns (bytes[] memory signatures)
    {
        address left = vm.addr(left_key);
        address right = vm.addr(right_key);
        signatures = new bytes[](2);
        if (uint160(left) < uint160(right)) {
            signatures[0] = _signature(left_key, digest);
            signatures[1] = _signature(right_key, digest);
        } else {
            signatures[0] = _signature(right_key, digest);
            signatures[1] = _signature(left_key, digest);
        }
    }

    function _signature(uint256 private_key, bytes32 digest) private returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(private_key, digest);
        return abi.encodePacked(r, s, v);
    }

    function _expectSubmitRevert(bytes[] memory signatures) private {
        try verifier.submitProof(PACKET_DIGEST, PFTL_HASH_COMMITMENT, PFTL_FINALIZED_HEIGHT, signatures) returns (
            bytes32
        ) {
            revert("expected submitProof revert");
        } catch {}
    }

    function _expectFinalizeRevert(bytes32 pending_id) private {
        try verifier.finalizeProof(pending_id) {
            revert("expected finalizeProof revert");
        } catch {}
    }

    function _expectChallengeRevert(bytes32 pending_id) private {
        try verifier.challengeProof(pending_id, PFTLWithdrawalVerifier.ChallengeFault.WrongPFTLHash) {
            revert("expected challengeProof revert");
        } catch {}
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) {
            revert(message);
        }
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEq(bytes32 actual, bytes32 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}
