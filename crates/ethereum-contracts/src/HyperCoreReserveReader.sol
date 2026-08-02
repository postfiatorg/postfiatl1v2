// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @notice Public, provider-neutral HyperEVM reader that commits HyperCore
/// account state into a canonical receipt log for NAV reserve verification.
contract HyperCoreReserveReader {
    address internal constant POSITION_PRECOMPILE = 0x0000000000000000000000000000000000000800;
    address internal constant SPOT_BALANCE_PRECOMPILE = 0x0000000000000000000000000000000000000801;
    address internal constant WITHDRAWABLE_PRECOMPILE = 0x0000000000000000000000000000000000000803;
    address internal constant MARK_PX_PRECOMPILE = 0x0000000000000000000000000000000000000806;
    address internal constant ACCOUNT_MARGIN_SUMMARY_PRECOMPILE = 0x000000000000000000000000000000000000080F;

    event HyperCoreSnapshot(bytes32 indexed commitment, uint64 blockTimeMs, bytes payload);

    struct Position {
        int64 szi;
        uint64 entryNtl;
        int64 isolatedRawUsd;
        uint32 leverage;
        bool isIsolated;
    }

    struct SpotBalance {
        uint64 total;
        uint64 hold;
        uint64 entryNtl;
    }

    struct AccountMarginSummary {
        int64 accountValue;
        uint64 marginUsed;
        uint64 ntlPos;
        int64 rawUsd;
    }

    struct SpotRequest {
        uint64 token;
        uint8 weiDecimals;
        uint32 priceAsset;
        uint8 priceAssetSzDecimals;
    }

    struct PerpSnapshot {
        uint32 perp;
        int64 szi;
        uint64 markPx;
    }

    struct SpotSnapshot {
        uint64 token;
        uint64 total;
        uint64 hold;
        uint8 weiDecimals;
        uint64 priceUsdE8;
    }

    /// @notice Reads official HyperCore precompiles for one account and emits
    /// one canonical receipt log.
    function snapshot(address account, uint32[] calldata perps, SpotRequest[] calldata spots, bytes32 salt)
        external
        returns (bytes32 commitment, bytes memory payload)
    {
        require(account != address(0), "ZERO_ACCOUNT");
        require(block.timestamp <= type(uint64).max / 1000, "TIMESTAMP_OVERFLOW");
        require(_strictlyIncreasingPerps(perps), "BAD_PERP_SET");
        require(_strictlyIncreasingSpots(spots), "BAD_SPOT_SET");
        // The public A666 successor is bound to the primary HyperCore perp DEX.
        // Do not accept caller-selected market state that the receipt payload
        // cannot independently disclose and bind.
        AccountMarginSummary memory marginSummary = _accountMarginSummary(0, account);
        uint64 withdrawable = _withdrawable(account);
        payload =
            abi.encode(account, marginSummary, withdrawable, _readPerps(account, perps), _readSpots(account, spots));
        commitment = keccak256(abi.encodePacked(payload, salt));
        emit HyperCoreSnapshot(commitment, uint64(block.timestamp * 1000), payload);
    }

    function _readPerps(address account, uint32[] calldata perps)
        internal
        view
        returns (PerpSnapshot[] memory snapshots)
    {
        snapshots = new PerpSnapshot[](perps.length);
        for (uint256 i = 0; i < perps.length; i++) {
            Position memory position = _position(account, perps[i]);
            snapshots[i] = PerpSnapshot({perp: perps[i], szi: position.szi, markPx: _markPx(perps[i])});
        }
    }

    function _readSpots(address account, SpotRequest[] calldata spots)
        internal
        view
        returns (SpotSnapshot[] memory snapshots)
    {
        snapshots = new SpotSnapshot[](spots.length);
        for (uint256 i = 0; i < spots.length; i++) {
            SpotBalance memory balance = _spotBalance(account, spots[i].token);
            snapshots[i] = SpotSnapshot({
                token: spots[i].token,
                total: balance.total,
                hold: balance.hold,
                weiDecimals: spots[i].weiDecimals,
                priceUsdE8: _spotPriceUsdE8(spots[i])
            });
        }
    }

    function _position(address account, uint32 perp) internal view returns (Position memory) {
        require(perp <= type(uint16).max, "PERP_INDEX_GT_UINT16");
        // forge-lint: disable-next-line(unsafe-typecast)
        (bool success, bytes memory result) = POSITION_PRECOMPILE.staticcall(abi.encode(account, uint16(perp)));
        require(success, "POSITION_READ_FAILED");
        return abi.decode(result, (Position));
    }

    function _spotBalance(address account, uint64 token) internal view returns (SpotBalance memory) {
        (bool success, bytes memory result) = SPOT_BALANCE_PRECOMPILE.staticcall(abi.encode(account, token));
        require(success, "SPOT_BALANCE_READ_FAILED");
        return abi.decode(result, (SpotBalance));
    }

    function _withdrawable(address account) internal view returns (uint64) {
        (bool success, bytes memory result) = WITHDRAWABLE_PRECOMPILE.staticcall(abi.encode(account));
        require(success, "WITHDRAWABLE_READ_FAILED");
        return abi.decode(result, (uint64));
    }

    function _markPx(uint32 asset) internal view returns (uint64) {
        (bool success, bytes memory result) = MARK_PX_PRECOMPILE.staticcall(abi.encode(asset));
        require(success, "MARK_PX_READ_FAILED");
        return abi.decode(result, (uint64));
    }

    function _accountMarginSummary(uint32 perpDexIndex, address account)
        internal
        view
        returns (AccountMarginSummary memory)
    {
        (bool success, bytes memory result) =
            ACCOUNT_MARGIN_SUMMARY_PRECOMPILE.staticcall(abi.encode(perpDexIndex, account));
        require(success, "ACCOUNT_MARGIN_SUMMARY_READ_FAILED");
        return abi.decode(result, (AccountMarginSummary));
    }

    function _spotPriceUsdE8(SpotRequest calldata spot) internal view returns (uint64) {
        if (spot.token == 404) {
            require(spot.weiDecimals == 8, "BAD_XMR1_DECIMALS");
            require(spot.priceAsset == 224, "BAD_XMR1_PRICE_ASSET");
            require(spot.priceAssetSzDecimals == 3, "BAD_XMR1_PRICE_DECIMALS");
            return _priceUsdE8(spot.priceAsset, spot.priceAssetSzDecimals);
        }
        if (spot.token == 150) {
            require(spot.weiDecimals == 8, "BAD_HYPE_DECIMALS");
            require(spot.priceAsset == 159, "BAD_HYPE_PRICE_ASSET");
            require(spot.priceAssetSzDecimals == 2, "BAD_HYPE_PRICE_DECIMALS");
            return _priceUsdE8(spot.priceAsset, spot.priceAssetSzDecimals);
        }
        revert("UNSUPPORTED_SPOT_TOKEN");
    }

    function _priceUsdE8(uint32 asset, uint8 szDecimals) internal view returns (uint64) {
        require(szDecimals <= 6, "BAD_SZ_DECIMALS");
        uint256 scale = 10 ** (2 + uint256(szDecimals));
        uint256 price = uint256(_markPx(asset)) * scale;
        require(price <= type(uint64).max, "PRICE_GT_UINT64");
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint64(price);
    }

    function _strictlyIncreasingPerps(uint32[] calldata perps) internal pure returns (bool) {
        for (uint256 i = 1; i < perps.length; i++) {
            if (perps[i - 1] >= perps[i]) return false;
        }
        return true;
    }

    function _strictlyIncreasingSpots(SpotRequest[] calldata spots) internal pure returns (bool) {
        for (uint256 i = 1; i < spots.length; i++) {
            if (spots[i - 1].token >= spots[i].token) return false;
        }
        return true;
    }
}
