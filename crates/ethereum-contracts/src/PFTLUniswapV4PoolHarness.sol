// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IExactInputRouter} from "./PFTLUniswapHandoffController.sol";

interface IERC20V4Harness {
    function balanceOf(address account) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

interface IPermit2V4Harness {
    function approve(address token, address spender, uint160 amount, uint48 expiration) external;
}

interface IPoolManagerV4Harness {
    function initialize(PoolKeyV4Harness calldata key, uint160 sqrtPriceX96) external returns (int24 tick);
    function unlock(bytes calldata data) external returns (bytes memory result);
    function swap(PoolKeyV4Harness calldata key, SwapParamsV4Harness calldata params, bytes calldata hookData)
        external
        returns (int256 delta);
    function sync(address currency) external;
    function settle() external payable returns (uint256 paid);
    function take(address currency, address to, uint256 amount) external;
}

interface IPositionManagerV4Harness {
    function modifyLiquidities(bytes calldata unlockData, uint256 deadline) external payable;
}

struct PoolKeyV4Harness {
    address currency0;
    address currency1;
    uint24 fee;
    int24 tickSpacing;
    address hooks;
}

struct SwapParamsV4Harness {
    bool zeroForOne;
    int256 amountSpecified;
    uint160 sqrtPriceLimitX96;
}

struct V4SwapData {
    PoolKeyV4Harness key;
    address tokenIn;
    address recipient;
    uint256 amountIn;
}

/// @notice Fork-only helper that initializes and seeds a hookless official Uniswap v4 pool.
/// @dev This is an evidence harness, not a public production controller. It mirrors the
///      already deployed StakeHub launch helper path and keeps the Gate 3 fork mechanics
///      reproducible inside this repository.
contract PFTLUniswapV4LaunchHelper {
    uint24 public constant FEE = 500;
    int24 public constant TICK_SPACING = 10;
    int24 public constant TICK_LOWER = -887270;
    int24 public constant TICK_UPPER = 887270;
    uint160 public constant Q96 = 79228162514264337593543950336;

    uint8 private constant ACTION_MINT_POSITION = 0x02;
    uint8 private constant ACTION_SETTLE_PAIR = 0x0d;

    address public immutable owner;
    IPoolManagerV4Harness public immutable poolManager;
    IPositionManagerV4Harness public immutable positionManager;
    IPermit2V4Harness public immutable permit2;

    event PoolInitialized(
        bytes32 indexed poolId, address indexed wrappedToken, address indexed usdcToken, uint160 sqrtPriceX96
    );
    event LiquiditySeeded(bytes32 indexed poolId, uint128 liquidity, uint256 amount0Max, uint256 amount1Max);

    error Unauthorized();
    error BadAmount();
    error BadCurrencyOrder();
    error LiquidityOverflow();
    error TransferFailed();

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    constructor(address owner_, address poolManager_, address positionManager_, address permit2_) {
        owner = owner_;
        poolManager = IPoolManagerV4Harness(poolManager_);
        positionManager = IPositionManagerV4Harness(positionManager_);
        permit2 = IPermit2V4Harness(permit2_);
    }

    function initializeAndSeed(address wrappedToken, address usdcToken, uint256 wrappedAmountMax, uint256 usdcAmountMax)
        external
        onlyOwner
        returns (bytes32 id, uint128 liquidity, uint160 sqrtPriceX96)
    {
        if (wrappedAmountMax == 0 || usdcAmountMax == 0) revert BadAmount();
        PoolKeyV4Harness memory key = poolKey(wrappedToken, usdcToken);
        sqrtPriceX96 = initialSqrtPriceX96(wrappedToken, usdcToken, wrappedAmountMax, usdcAmountMax);
        poolManager.initialize(key, sqrtPriceX96);
        emit PoolInitialized(poolId(key), wrappedToken, usdcToken, sqrtPriceX96);

        uint256 amount0Max = key.currency0 == wrappedToken ? wrappedAmountMax : usdcAmountMax;
        uint256 amount1Max = key.currency0 == wrappedToken ? usdcAmountMax : wrappedAmountMax;
        liquidity = liquidityForAmounts(sqrtPriceX96, amount0Max, amount1Max);
        if (liquidity == 0) revert BadAmount();

        approvePeriphery(key.currency0, address(positionManager));
        approvePeriphery(key.currency1, address(positionManager));

        bytes memory actions = abi.encodePacked(ACTION_MINT_POSITION, ACTION_SETTLE_PAIR);
        bytes[] memory params = new bytes[](2);
        params[0] = abi.encode(
            key, TICK_LOWER, TICK_UPPER, uint256(liquidity), uint128(amount0Max), uint128(amount1Max), owner, bytes("")
        );
        params[1] = abi.encode(key.currency0, key.currency1);
        positionManager.modifyLiquidities(abi.encode(actions, params), block.timestamp + 600);

        id = poolId(key);
        emit LiquiditySeeded(id, liquidity, amount0Max, amount1Max);
    }

    function poolKey(address tokenA, address tokenB) public pure returns (PoolKeyV4Harness memory key) {
        if (tokenA == tokenB || tokenA == address(0) || tokenB == address(0)) revert BadCurrencyOrder();
        (address currency0, address currency1) = uint160(tokenA) < uint160(tokenB) ? (tokenA, tokenB) : (tokenB, tokenA);
        key = PoolKeyV4Harness({
            currency0: currency0, currency1: currency1, fee: FEE, tickSpacing: TICK_SPACING, hooks: address(0)
        });
    }

    function poolId(PoolKeyV4Harness memory key) public pure returns (bytes32) {
        return keccak256(abi.encode(key.currency0, key.currency1, key.fee, key.tickSpacing, key.hooks));
    }

    function initialSqrtPriceX96(address wrappedToken, address usdcToken, uint256 wrappedAmount, uint256 usdcAmount)
        public
        pure
        returns (uint160)
    {
        if (wrappedAmount == 0 || usdcAmount == 0) revert BadAmount();
        PoolKeyV4Harness memory key = poolKey(wrappedToken, usdcToken);
        uint256 amount0 = key.currency0 == wrappedToken ? wrappedAmount : usdcAmount;
        uint256 amount1 = key.currency0 == wrappedToken ? usdcAmount : wrappedAmount;
        uint256 ratioX192 = mulDiv(amount1, uint256(Q96) ** 2, amount0);
        uint256 sqrtRatio = sqrt(ratioX192);
        if (sqrtRatio > type(uint160).max) revert BadAmount();
        return uint160(sqrtRatio);
    }

    function liquidityForAmounts(uint160 sqrtPriceX96, uint256 amount0, uint256 amount1)
        public
        pure
        returns (uint128 liquidity)
    {
        uint256 liquidity0 = mulDiv(amount0, uint256(sqrtPriceX96), Q96);
        uint256 liquidity1 = mulDiv(amount1, Q96, uint256(sqrtPriceX96));
        uint256 selected = liquidity0 < liquidity1 ? liquidity0 : liquidity1;
        if (selected > type(uint128).max) revert LiquidityOverflow();
        liquidity = uint128(selected);
    }

    function approvePeriphery(address token, address spender) private {
        if (!IERC20V4Harness(token).approve(address(permit2), type(uint256).max)) revert TransferFailed();
        permit2.approve(token, spender, type(uint160).max, type(uint48).max);
    }

    function sqrt(uint256 value) internal pure returns (uint256 result) {
        if (value == 0) return 0;
        uint256 candidate = (value + 1) / 2;
        result = value;
        while (candidate < result) {
            result = candidate;
            candidate = (value / candidate + candidate) / 2;
        }
    }

    function mulDiv(uint256 a, uint256 b, uint256 denominator) internal pure returns (uint256 result) {
        uint256 prod0;
        uint256 prod1;
        assembly {
            let mm := mulmod(a, b, not(0))
            prod0 := mul(a, b)
            prod1 := sub(sub(mm, prod0), lt(mm, prod0))
        }

        if (prod1 == 0) {
            return prod0 / denominator;
        }
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

/// @notice Exact-input router shim that settles against the official v4 PoolManager.
/// @dev Intended for controlled fork rehearsal of the bridge settlement adapter.
contract PFTLUniswapV4ExactInputRouter is IExactInputRouter {
    uint24 public constant FEE = 500;
    int24 public constant TICK_SPACING = 10;
    uint160 public constant MIN_SQRT_PRICE_PLUS_ONE = 4295128740;
    uint160 public constant MAX_SQRT_PRICE_MINUS_ONE = 1461446703485210103287273052203988822378723970341;

    IPoolManagerV4Harness public immutable poolManager;

    event ExactInputV4(
        address indexed tokenIn,
        address indexed tokenOut,
        address indexed recipient,
        uint256 amountIn,
        uint256 amountOut
    );

    error BadAmount();
    error BadCurrencyOrder();
    error BadCallback();
    error DeadlineExpired(uint256 nowTimestamp, uint256 deadline);
    error TransferFailed();

    constructor(address poolManager_) {
        poolManager = IPoolManagerV4Harness(poolManager_);
    }

    function exactInput(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minimumOutput,
        address recipient,
        uint256 deadline,
        bytes calldata
    ) external returns (uint256 amountOut) {
        if (block.timestamp > deadline) revert DeadlineExpired(block.timestamp, deadline);
        if (amountIn == 0) revert BadAmount();
        PoolKeyV4Harness memory key = poolKey(tokenIn, tokenOut);
        uint256 beforeOut = IERC20V4Harness(tokenOut).balanceOf(recipient);
        if (!IERC20V4Harness(tokenIn).transferFrom(msg.sender, address(this), amountIn)) revert TransferFailed();
        poolManager.unlock(
            abi.encode(V4SwapData({key: key, tokenIn: tokenIn, recipient: recipient, amountIn: amountIn}))
        );
        amountOut = IERC20V4Harness(tokenOut).balanceOf(recipient) - beforeOut;
        if (amountOut < minimumOutput) revert BadAmount();
        emit ExactInputV4(tokenIn, tokenOut, recipient, amountIn, amountOut);
    }

    function unlockCallback(bytes calldata data) external returns (bytes memory result) {
        if (msg.sender != address(poolManager)) revert BadCallback();
        V4SwapData memory swapData = abi.decode(data, (V4SwapData));
        bool zeroForOne = swapData.tokenIn == swapData.key.currency0;
        SwapParamsV4Harness memory params = SwapParamsV4Harness({
            zeroForOne: zeroForOne,
            amountSpecified: -int256(swapData.amountIn),
            sqrtPriceLimitX96: zeroForOne ? MIN_SQRT_PRICE_PLUS_ONE : MAX_SQRT_PRICE_MINUS_ONE
        });
        int256 delta = poolManager.swap(swapData.key, params, bytes(""));
        _settleAndTake(swapData.key, delta, swapData.recipient);
        result = abi.encode(delta);
    }

    function poolKey(address tokenA, address tokenB) public pure returns (PoolKeyV4Harness memory key) {
        if (tokenA == tokenB || tokenA == address(0) || tokenB == address(0)) revert BadCurrencyOrder();
        (address currency0, address currency1) = uint160(tokenA) < uint160(tokenB) ? (tokenA, tokenB) : (tokenB, tokenA);
        key = PoolKeyV4Harness({
            currency0: currency0, currency1: currency1, fee: FEE, tickSpacing: TICK_SPACING, hooks: address(0)
        });
    }

    function _settleAndTake(PoolKeyV4Harness memory key, int256 delta, address recipient) private {
        int128 delta0 = int128(delta >> 128);
        int128 delta1 = int128(delta);
        if (delta0 < 0) _settleCurrency(key.currency0, uint256(uint128(-delta0)));
        if (delta1 < 0) _settleCurrency(key.currency1, uint256(uint128(-delta1)));
        if (delta0 > 0) poolManager.take(key.currency0, recipient, uint256(uint128(delta0)));
        if (delta1 > 0) poolManager.take(key.currency1, recipient, uint256(uint128(delta1)));
    }

    function _settleCurrency(address currency, uint256 amount) private {
        poolManager.sync(currency);
        if (!IERC20V4Harness(currency).transfer(address(poolManager), amount)) revert TransferFailed();
        poolManager.settle();
    }
}
