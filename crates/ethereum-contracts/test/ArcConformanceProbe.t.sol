// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ArcConformanceProbe} from "../src/ArcConformanceProbe.sol";

contract ArcConformanceProbeTest {
    ArcConformanceProbe private probe;

    bytes32 private constant TWO_G_X = 0x030644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd3;
    bytes32 private constant TWO_G_Y = 0x15ed738c0e0a7c92e7845f96b2ae9c0a68a6a449e3538fc7ff3ebf7a5a18a2c4;

    function setUp() public {
        probe = new ArcConformanceProbe();
    }

    function testSha256KnownVector() public view {
        (bytes32 digest,) = probe.sha256Probe(bytes("abc"));
        require(digest == sha256(bytes("abc")), "SHA-256 precompile mismatch");
        require(
            digest == 0xba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad,
            "SHA-256 known vector mismatch"
        );
    }

    function testBn254AddAndMulKnownVector() public view {
        bytes memory addInput = abi.encode(uint256(1), uint256(2), uint256(1), uint256(2));
        bytes memory mulInput = abi.encode(uint256(1), uint256(2), uint256(2));
        (bytes memory addOutput,) = probe.bn254Add(addInput);
        (bytes memory mulOutput,) = probe.bn254Mul(mulInput);
        bytes memory expected = abi.encode(TWO_G_X, TWO_G_Y);
        require(keccak256(addOutput) == keccak256(expected), "BN254 add known vector mismatch");
        require(keccak256(mulOutput) == keccak256(expected), "BN254 mul known vector mismatch");
    }

    function testBn254EmptyPairingProductIsOne() public view {
        (bool valid,) = probe.bn254Pairing("");
        require(valid, "BN254 empty pairing product must be one");
    }

    function testRejectsMalformedPairingInput() public view {
        try probe.bn254Pairing(hex"01") returns (bool, uint256) {
            revert("malformed pairing input unexpectedly accepted");
        } catch {}
    }
}
