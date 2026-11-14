// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;
interface ISP1Verifier {
    function verifyProof(bytes32, bytes calldata, bytes calldata) external view;
}
interface ISP1VerifierWithHash is ISP1Verifier {
    function VERIFIER_HASH() external pure returns (bytes32);
}
