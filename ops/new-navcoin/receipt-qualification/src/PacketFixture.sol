// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;
import {PFTLUniswapPrimaryMarketV2} from "./PFTLUniswapPrimaryMarketV2.sol";
import {PFTLReturnRouteV3} from "./PFTLReturnRouteV3.sol";
library RealReceiptPacketFixture {
    function packet() internal pure returns (PFTLUniswapPrimaryMarketV2.MintPacket memory) {
        return PFTLUniswapPrimaryMarketV2.MintPacket({routeConfigDigest: hex"777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777",
sourcePacketHash: hex"818181818181818181818181818181818181818181818181818181818181818181818181818181818181818181818181",
reservationId: hex"828282828282828282828282828282828282828282828282828282828282828282828282828282828282828282828282",
sourceReceiptHash: hex"e00c0c50d7b95c3c5d24b43a00de0ffb3735f37283a7f3f4d2c8ff522ce10cc9b990c3fc28fad6866c6b14fa56b78406",
sourceReceiptRoot: hex"57937457814482df5b36e6aacb6987946503b24e7a296b8094f2edd10be45c7e0aeb5057703b7561e12d783136d6172f",
settlementAssetId: hex"838383838383838383838383838383838383838383838383838383838383838383838383838383838383838383838383",
nativeNavAssetId: hex"848484848484848484848484848484848484848484848484848484848484848484848484848484848484848484848484",
pricingReservePacketHash: hex"555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555",
policyHashCommitment: bytes32(hex"ce94a7eda9d295875fd931624e338b1dfcf2e0b78ebdb0bef2ffba340a785f82"),
nonce: bytes32(hex"8585858585858585858585858585858585858585858585858585858585858585"),
routeEpoch: 1,
pricingNavEpoch: 7,
deadline: 1800000000,
destinationChainId: 1,
mintAmountAtoms: 100,
settlementValueAtoms: 101,
destinationController: address(bytes20(hex"1111111111111111111111111111111111111111")),
wrappedToken: address(bytes20(hex"3333333333333333333333333333333333333333")),
ethereumRecipient: address(bytes20(hex"4444444444444444444444444444444444444444"))});
    }
    function domain() internal pure returns (PFTLReturnRouteV3.ReturnRouteDomain memory) {
        return PFTLReturnRouteV3.ReturnRouteDomain({chain_id: string(hex"706f7374666961742d726563656970742d66697874757265"), genesis_hash: hex"abababababababababababababababababababababababababababababababababababababababababababababababab", protocol_version: 1, route_id: string(hex"726563656970742d70726f6f662d66697874757265")});
    }
}
