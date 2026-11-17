// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @dev ABI domain shared by the two V3 controller successors and the L1 verifier.
abstract contract PFTLReturnRouteV3 {
    struct ReturnRouteDomain {
        string chain_id;
        bytes genesis_hash;
        uint32 protocol_version;
        string route_id;
    }
    bytes32 internal immutable return_route_binding;

    constructor(ReturnRouteDomain memory domain) {
        require(bytes(domain.chain_id).length != 0 && bytes(domain.route_id).length != 0, "empty return domain");
        require(domain.genesis_hash.length == 48 && domain.protocol_version != 0, "invalid return domain");
        require(keccak256(domain.genesis_hash) != keccak256(new bytes(48)), "zero genesis");
        return_route_binding = keccak256(abi.encode(
            "postfiat.pftl_uniswap.return_route.v3", domain.chain_id, domain.genesis_hash,
            domain.protocol_version, domain.route_id
        ));
        require(return_route_binding != bytes32(0), "zero return binding");
    }
}
