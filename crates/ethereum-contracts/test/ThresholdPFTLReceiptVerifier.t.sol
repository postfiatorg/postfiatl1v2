// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ThresholdPFTLReceiptVerifier} from "../src/PFTLUniswapHandoffController.sol";

interface ThresholdVerifierVm {
    function addr(uint256 private_key) external returns (address);
    function sign(uint256 private_key, bytes32 digest) external returns (uint8 v, bytes32 r, bytes32 s);
    function expectRevert() external;
    function expectRevert(bytes4 selector) external;
}

contract ThresholdPFTLReceiptVerifierTest {
    ThresholdVerifierVm private constant vm =
        ThresholdVerifierVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    bytes private constant RECEIPT_ROOT =
        hex"111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111";
    bytes private constant RECEIPT_HASH =
        hex"222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222";
    bytes private constant ROUTE_CONFIG =
        hex"333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333";
    bytes32 private constant PACKET_DIGEST = keccak256("packet-1");

    ThresholdPFTLReceiptVerifier private verifier;
    uint256[] private signer_keys;

    function setUp() public {
        signer_keys.push(0xA101);
        signer_keys.push(0xA102);
        signer_keys.push(0xA103);
        signer_keys.push(0xA104);
        _sortKeysByAddress(signer_keys);
        address[] memory signers = _signerAddresses(signer_keys);
        verifier = new ThresholdPFTLReceiptVerifier(
            keccak256("postfiat-devnet"),
            keccak256(
                hex"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            1,
            7,
            signers,
            3
        );
    }

    function testExactBftCertificateAcceptsReceiptOnce() public {
        bytes32 code = verifier.ACCEPTED_RECEIPT_CODE();
        bytes32 digest = verifier.certificateDigest(RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, PACKET_DIGEST, 1212, code);
        bytes[] memory signatures = _signatures(digest, 3);

        bytes32 commitment = verifier.submitReceiptCertificate(
            RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, PACKET_DIGEST, 1212, code, signatures
        );

        require(commitment != bytes32(0), "zero receipt commitment");
        require(
            verifier.isReceiptAccepted(
                RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, verifier.TRUST_CLASS_BFT_CHECKPOINT(), PACKET_DIGEST
            ),
            "certified receipt not accepted"
        );

        vm.expectRevert();
        verifier.submitReceiptCertificate(
            RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, PACKET_DIGEST, 1212, code, signatures
        );
    }

    function testUnderQuorumDuplicateWrongCodeAndTamperingFailClosed() public {
        bytes32 code = verifier.ACCEPTED_RECEIPT_CODE();
        bytes32 digest = verifier.certificateDigest(RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, PACKET_DIGEST, 1212, code);

        vm.expectRevert();
        verifier.submitReceiptCertificate(
            RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, PACKET_DIGEST, 1212, code, _signatures(digest, 2)
        );

        bytes[] memory duplicate = new bytes[](3);
        duplicate[0] = _signature(signer_keys[0], digest);
        duplicate[1] = _signature(signer_keys[0], digest);
        duplicate[2] = _signature(signer_keys[1], digest);
        vm.expectRevert();
        verifier.submitReceiptCertificate(
            RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, PACKET_DIGEST, 1212, code, duplicate
        );

        vm.expectRevert();
        verifier.submitReceiptCertificate(
            RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, PACKET_DIGEST, 1212, keccak256("rejected"), _signatures(digest, 3)
        );

        vm.expectRevert();
        verifier.submitReceiptCertificate(
            RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, keccak256("different-packet"), 1212, code, _signatures(digest, 3)
        );

        require(
            !verifier.isReceiptAccepted(
                RECEIPT_ROOT, RECEIPT_HASH, ROUTE_CONFIG, verifier.TRUST_CLASS_BFT_CHECKPOINT(), PACKET_DIGEST
            ),
            "failed submissions mutated acceptance"
        );
    }

    function testConstructorRejectsThresholdLooseningAndUnsortedCommittee() public {
        address[] memory signers = _signerAddresses(signer_keys);
        vm.expectRevert();
        new ThresholdPFTLReceiptVerifier(keccak256("postfiat-devnet"), keccak256("genesis"), 1, 7, signers, 2);

        (signers[0], signers[1]) = (signers[1], signers[0]);
        vm.expectRevert();
        new ThresholdPFTLReceiptVerifier(keccak256("postfiat-devnet"), keccak256("genesis"), 1, 7, signers, 3);
    }

    function _signatures(bytes32 digest, uint256 count) private returns (bytes[] memory signatures) {
        signatures = new bytes[](count);
        for (uint256 i = 0; i < count; i++) {
            signatures[i] = _signature(signer_keys[i], digest);
        }
    }

    function _signature(uint256 private_key, bytes32 digest) private returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(private_key, digest);
        return abi.encodePacked(r, s, v);
    }

    function _signerAddresses(uint256[] storage keys) private returns (address[] memory signers) {
        signers = new address[](keys.length);
        for (uint256 i = 0; i < keys.length; i++) {
            signers[i] = vm.addr(keys[i]);
        }
    }

    function _sortKeysByAddress(uint256[] storage keys) private {
        for (uint256 i = 1; i < keys.length; i++) {
            uint256 key = keys[i];
            address signer = vm.addr(key);
            uint256 cursor = i;
            while (cursor > 0 && vm.addr(keys[cursor - 1]) > signer) {
                keys[cursor] = keys[cursor - 1];
                cursor -= 1;
            }
            keys[cursor] = key;
        }
    }
}
