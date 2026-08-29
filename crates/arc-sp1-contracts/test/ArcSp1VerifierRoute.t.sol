// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";
import {ISP1VerifierGatewayErrors} from "@sp1-contracts/ISP1VerifierGateway.sol";
import {SP1Verifier} from "@sp1-contracts/v6.1.0/SP1VerifierGroth16.sol";

contract ArcSp1VerifierRouteTest {
    bytes32 private constant EXPECTED_HASH =
        0x4388a21c687fdd5f218d7e3d13190cac4c5355818d3605fd5fb811df468ee696;
    // forge-lint: disable-next-line(unsafe-typecast)
    bytes4 private constant EXPECTED_SELECTOR = bytes4(EXPECTED_HASH);

    SP1VerifierGateway private gateway;
    SP1Verifier private verifier;

    function setUp() public {
        gateway = new SP1VerifierGateway(address(this));
        verifier = new SP1Verifier();
        gateway.addRoute(address(verifier));
    }

    function testPinnedSdkRouteIsInstalledAndUsable() public view {
        require(keccak256(bytes(verifier.VERSION())) == keccak256(bytes("v6.1.0")), "version");
        require(verifier.VERIFIER_HASH() == EXPECTED_HASH, "verifier hash");
        (address routedVerifier, bool frozen) = gateway.routes(EXPECTED_SELECTOR);
        require(routedVerifier == address(verifier), "route verifier");
        require(!frozen, "route frozen");
    }

    function testUnknownSelectorFailsClosedAtGateway() public {
        bytes memory proof = abi.encodePacked(bytes4(0xdeadbeef), new bytes(352));
        (bool ok, bytes memory revertData) = address(gateway)
            .call(abi.encodeCall(gateway.verifyProof, (bytes32(uint256(1)), hex"01", proof)));
        require(!ok, "unknown selector accepted");
        require(
            _revertSelector(revertData) == ISP1VerifierGatewayErrors.RouteNotFound.selector,
            "wrong gateway error"
        );
    }

    function testPinnedSelectorRoutesAndMalformedProofFailsInVerifier() public {
        bytes memory proof = abi.encodePacked(EXPECTED_SELECTOR, new bytes(352));
        (bool ok,) = address(gateway)
            .call(abi.encodeCall(gateway.verifyProof, (bytes32(uint256(1)), hex"01", proof)));
        require(!ok, "malformed proof accepted");
    }

    function testDuplicateRouteCannotReplacePinnedVerifier() public {
        (bool ok, bytes memory revertData) =
            address(gateway).call(abi.encodeCall(gateway.addRoute, (address(verifier))));
        require(!ok, "route replaced");
        require(
            _revertSelector(revertData) == ISP1VerifierGatewayErrors.RouteAlreadyExists.selector,
            "wrong duplicate error"
        );
    }

    function _revertSelector(bytes memory revertData) private pure returns (bytes4 selector) {
        if (revertData.length < 4) return bytes4(0);
        assembly ("memory-safe") {
            selector := mload(add(revertData, 0x20))
        }
    }
}
