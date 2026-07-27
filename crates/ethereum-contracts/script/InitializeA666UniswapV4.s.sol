// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    IERC20V4Harness,
    IPermit2V4Harness,
    IPoolManagerV4Harness,
    IPositionManagerV4Harness,
    PoolKeyV4Harness
} from "../src/PFTLUniswapV4PoolHarness.sol";

interface InitializeA666Vm {
    function envAddress(string calldata name) external returns (address);
    function envBytes32(string calldata name) external returns (bytes32);
    function envUint(string calldata name) external returns (uint256);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

/// @notice Direct EOA pool initialization/seed script; deploys no helper.
contract InitializeA666UniswapV4 {
    InitializeA666Vm private constant vm = InitializeA666Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    address private constant MAINNET_USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    uint24 private constant FEE = 500;
    int24 private constant TICK_SPACING = 10;
    int24 private constant TICK_LOWER = -887270;
    int24 private constant TICK_UPPER = 887270;
    uint160 private constant Q96 = 79228162514264337593543950336;
    uint8 private constant ACTION_MINT_POSITION = 0x02;
    uint8 private constant ACTION_SETTLE_PAIR = 0x0d;

    error WrongChain(uint256 actual);
    error WrongUsdc(address actual);
    error PoolIdMismatch(bytes32 expected, bytes32 actual);
    error RuntimeCodeHashMismatch(string component, bytes32 expected, bytes32 actual);
    error BadAmount();
    error TransferFailed();
    error Uint128Overflow(uint256 value);

    event A666PoolInitializedAndSeeded(bytes32 indexed poolId, address indexed wrappedToken);

    struct SeedConfig {
        address wrappedToken;
        address poolManager;
        address positionManager;
        address permit2;
        uint256 wrappedAmountMax;
        uint256 usdcAmountMax;
        PoolKeyV4Harness key;
    }

    function run() external returns (bytes32 poolId, uint160 sqrtPriceX96, uint128 liquidity) {
        if (block.chainid != 1) revert WrongChain(block.chainid);
        SeedConfig memory config = _loadConfig();
        sqrtPriceX96 =
            _initialSqrtPriceX96(config.key, config.wrappedToken, config.wrappedAmountMax, config.usdcAmountMax);
        poolId = keccak256(
            abi.encode(
                config.key.currency0, config.key.currency1, config.key.fee, config.key.tickSpacing, config.key.hooks
            )
        );
        bytes32 expectedPoolId = vm.envBytes32("A666_UNISWAP_POOL_ID");
        if (poolId != expectedPoolId) revert PoolIdMismatch(expectedPoolId, poolId);
        liquidity =
            _liquidity(config.key, config.wrappedToken, sqrtPriceX96, config.wrappedAmountMax, config.usdcAmountMax);

        vm.startBroadcast(vm.envUint("PRIVATE_KEY"));
        _initializeAndSeed(config, sqrtPriceX96, liquidity);
        vm.stopBroadcast();
        emit A666PoolInitializedAndSeeded(poolId, config.wrappedToken);
    }

    function _loadConfig() private returns (SeedConfig memory config) {
        address wrappedToken = vm.envAddress("A666_WRAPPED_TOKEN");
        address usdc = vm.envAddress("A666_MAINNET_USDC");
        if (usdc != MAINNET_USDC) revert WrongUsdc(usdc);
        config.wrappedToken = wrappedToken;
        config.poolManager = vm.envAddress("UNISWAP_V4_POOL_MANAGER");
        config.positionManager = vm.envAddress("UNISWAP_V4_POSITION_MANAGER");
        config.permit2 = vm.envAddress("UNISWAP_PERMIT2");
        _requireCodeHash("pool_manager", config.poolManager, vm.envBytes32("UNISWAP_V4_POOL_MANAGER_CODE_HASH"));
        _requireCodeHash(
            "position_manager", config.positionManager, vm.envBytes32("UNISWAP_V4_POSITION_MANAGER_CODE_HASH")
        );
        _requireCodeHash("permit2", config.permit2, vm.envBytes32("UNISWAP_PERMIT2_CODE_HASH"));

        config.wrappedAmountMax = vm.envUint("A666_POOL_WRAPPED_AMOUNT_MAX");
        config.usdcAmountMax = vm.envUint("A666_POOL_USDC_AMOUNT_MAX");
        if (config.wrappedAmountMax == 0 || config.usdcAmountMax == 0) revert BadAmount();
        config.key = _poolKey(wrappedToken, usdc);
    }

    function _initializeAndSeed(SeedConfig memory config, uint160 sqrtPriceX96, uint128 liquidity) private {
        IPoolManagerV4Harness(config.poolManager).initialize(config.key, sqrtPriceX96);
        _approve(config.key.currency0, config.permit2, config.positionManager);
        _approve(config.key.currency1, config.permit2, config.positionManager);
        uint256 amount0Max =
            config.key.currency0 == config.wrappedToken ? config.wrappedAmountMax : config.usdcAmountMax;
        uint256 amount1Max =
            config.key.currency0 == config.wrappedToken ? config.usdcAmountMax : config.wrappedAmountMax;
        bytes memory actions = abi.encodePacked(ACTION_MINT_POSITION, ACTION_SETTLE_PAIR);
        bytes[] memory params = new bytes[](2);
        params[0] = abi.encode(
            config.key,
            TICK_LOWER,
            TICK_UPPER,
            uint256(liquidity),
            _uint128(amount0Max),
            _uint128(amount1Max),
            vm.envAddress("A666_POOL_POSITION_OWNER"),
            bytes("")
        );
        params[1] = abi.encode(config.key.currency0, config.key.currency1);
        IPositionManagerV4Harness(config.positionManager)
            .modifyLiquidities(abi.encode(actions, params), block.timestamp + 600);
        _revoke(config.key.currency0, config.permit2, config.positionManager);
        _revoke(config.key.currency1, config.permit2, config.positionManager);
    }

    function _poolKey(address tokenA, address tokenB) private pure returns (PoolKeyV4Harness memory) {
        if (tokenA == address(0) || tokenA == tokenB) revert BadAmount();
        (address currency0, address currency1) = uint160(tokenA) < uint160(tokenB) ? (tokenA, tokenB) : (tokenB, tokenA);
        return PoolKeyV4Harness({
            currency0: currency0, currency1: currency1, fee: FEE, tickSpacing: TICK_SPACING, hooks: address(0)
        });
    }

    function _initialSqrtPriceX96(
        PoolKeyV4Harness memory key,
        address wrappedToken,
        uint256 wrappedAmount,
        uint256 usdcAmount
    ) private pure returns (uint160) {
        uint256 amount0 = key.currency0 == wrappedToken ? wrappedAmount : usdcAmount;
        uint256 amount1 = key.currency0 == wrappedToken ? usdcAmount : wrappedAmount;
        uint256 ratioX192 = _mulDiv(amount1, uint256(Q96) ** 2, amount0);
        uint256 value = _sqrt(ratioX192);
        if (value > type(uint160).max) revert BadAmount();
        return uint160(value);
    }

    function _liquidity(
        PoolKeyV4Harness memory key,
        address wrappedToken,
        uint160 sqrtPriceX96,
        uint256 wrappedAmount,
        uint256 usdcAmount
    ) private pure returns (uint128) {
        uint256 amount0 = key.currency0 == wrappedToken ? wrappedAmount : usdcAmount;
        uint256 amount1 = key.currency0 == wrappedToken ? usdcAmount : wrappedAmount;
        uint256 liquidity0 = _mulDiv(amount0, uint256(sqrtPriceX96), Q96);
        uint256 liquidity1 = _mulDiv(amount1, Q96, uint256(sqrtPriceX96));
        uint256 selected = liquidity0 < liquidity1 ? liquidity0 : liquidity1;
        return _uint128(selected);
    }

    function _approve(address token, address permit2, address positionManager) private {
        if (!IERC20V4Harness(token).approve(permit2, type(uint256).max)) revert TransferFailed();
        IPermit2V4Harness(permit2).approve(token, positionManager, type(uint160).max, type(uint48).max);
    }

    function _revoke(address token, address permit2, address positionManager) private {
        IPermit2V4Harness(permit2).approve(token, positionManager, 0, 0);
        if (!IERC20V4Harness(token).approve(permit2, 0)) revert TransferFailed();
    }

    function _requireCodeHash(string memory component, address target, bytes32 expected) private view {
        bytes32 actual = target.codehash;
        if (expected == bytes32(0) || actual != expected) {
            revert RuntimeCodeHashMismatch(component, expected, actual);
        }
    }

    function _uint128(uint256 value) private pure returns (uint128) {
        if (value > type(uint128).max) revert Uint128Overflow(value);
        return uint128(value);
    }

    function _sqrt(uint256 value) private pure returns (uint256 result) {
        if (value == 0) return 0;
        uint256 candidate = (value + 1) / 2;
        result = value;
        while (candidate < result) {
            result = candidate;
            candidate = (value / candidate + candidate) / 2;
        }
    }

    function _mulDiv(uint256 a, uint256 b, uint256 denominator) private pure returns (uint256 result) {
        uint256 prod0;
        uint256 prod1;
        assembly {
            let mm := mulmod(a, b, not(0))
            prod0 := mul(a, b)
            prod1 := sub(sub(mm, prod0), lt(mm, prod0))
        }
        if (prod1 == 0) return prod0 / denominator;
        require(denominator > prod1, "mulDiv overflow");
        uint256 remainder;
        assembly {
            remainder := mulmod(a, b, denominator)
            prod1 := sub(prod1, gt(remainder, prod0))
            prod0 := sub(prod0, remainder)
        }
        uint256 twos = denominator & (~denominator + 1);
        assembly {
            denominator := div(denominator, twos)
            prod0 := div(prod0, twos)
            twos := add(div(sub(0, twos), twos), 1)
        }
        unchecked {
            prod0 |= prod1 * twos;
            uint256 inverse = (3 * denominator) ^ 2;
            inverse *= 2 - denominator * inverse;
            inverse *= 2 - denominator * inverse;
            inverse *= 2 - denominator * inverse;
            inverse *= 2 - denominator * inverse;
            inverse *= 2 - denominator * inverse;
            inverse *= 2 - denominator * inverse;
            result = prod0 * inverse;
        }
    }
}
