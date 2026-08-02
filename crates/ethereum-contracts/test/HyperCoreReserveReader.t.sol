// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {HyperCoreReserveReader} from "../src/HyperCoreReserveReader.sol";

contract HyperCoreReserveReaderTest {
    HyperCoreReserveReader internal reader;

    function setUp() public {
        reader = new HyperCoreReserveReader();
    }

    function testRejectsZeroAccountBeforeReadingPrecompiles() public {
        uint32[] memory perps = new uint32[](0);
        HyperCoreReserveReader.SpotRequest[] memory spots = new HyperCoreReserveReader.SpotRequest[](0);
        (bool ok,) =
            address(reader).call(abi.encodeCall(reader.snapshot, (address(0), perps, spots, bytes32(uint256(1)))));
        require(!ok, "zero account accepted");
    }

    function testRejectsDuplicateOrUnsortedPerpSetBeforeReadingPrecompiles() public {
        uint32[] memory perps = new uint32[](2);
        perps[0] = 7;
        perps[1] = 7;
        HyperCoreReserveReader.SpotRequest[] memory spots = new HyperCoreReserveReader.SpotRequest[](0);
        (bool ok,) =
            address(reader).call(abi.encodeCall(reader.snapshot, (address(1), perps, spots, bytes32(uint256(1)))));
        require(!ok, "duplicate perp accepted");
    }

    function testRejectsDuplicateOrUnsortedSpotSetBeforeReadingPrecompiles() public {
        uint32[] memory perps = new uint32[](0);
        HyperCoreReserveReader.SpotRequest[] memory spots = new HyperCoreReserveReader.SpotRequest[](2);
        spots[0] =
            HyperCoreReserveReader.SpotRequest({token: 404, weiDecimals: 8, priceAsset: 224, priceAssetSzDecimals: 3});
        spots[1] = spots[0];
        (bool ok,) =
            address(reader).call(abi.encodeCall(reader.snapshot, (address(1), perps, spots, bytes32(uint256(1)))));
        require(!ok, "duplicate spot accepted");
    }
}
