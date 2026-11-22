// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTLUniswapPrimaryMarketV2, IA666WrappedToken, IPFTLReceiptFinalityVerifierV1}
    from "./PFTLUniswapPrimaryMarketV2.sol";
import {PFTLReturnRouteV3} from "./PFTLReturnRouteV3.sol";

/// @notice Primary-market controller with immutable L1 destination binding on every return.
contract PFTLUniswapPrimaryMarketV3 is PFTLUniswapPrimaryMarketV2, PFTLReturnRouteV3 {
    constructor(
        IA666WrappedToken token, IPFTLReceiptFinalityVerifierV1 verifier,
        Config memory config, ReturnRouteDomain memory domain
    ) PFTLUniswapPrimaryMarketV2(token, verifier, config) PFTLReturnRouteV3(domain) {}

    function returnRouteBinding() public view override returns (bytes32) { return return_route_binding; }
}
