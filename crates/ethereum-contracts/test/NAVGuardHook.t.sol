// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {NAVGuardHook} from "../src/NAVGuardHook.sol";

interface HookVm {
    function warp(uint256 timestamp) external;
}

contract NAVGuardHookTest {
    HookVm private constant vm = HookVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    MockPoolManager private manager;
    NAVGuardHook private hook;

    bytes32 private constant POOL_ID = bytes32(uint256(0xabc));
    bytes32 private constant POOL_CONFIG_HASH = bytes32(uint256(0x1234));
    bytes32 private constant PFTL_STATE_HASH = bytes32(uint256(0x5678));

    function setUp() public {
        manager = new MockPoolManager();
        hook = new NAVGuardHook(address(manager), address(this), 4, 4, 1, 300);
        manager.setHook(hook);

        hook.registerPool(POOL_ID, POOL_CONFIG_HASH);
        vm.warp(1_000);
        hook.updatePFTLState(POOL_ID, PFTL_STATE_HASH);
    }

    function testSwapProducesObservation() public {
        bytes4 before_selector = manager.beforeSwap(POOL_ID);
        _assertEqBytes4(before_selector, NAVGuardHook.beforeSwap.selector, "before selector");

        bytes4 after_selector = manager.afterSwap(
            NAVGuardHook.SwapObservationInput({
                pool_id: POOL_ID,
                price_usd_e8: 475_000_000,
                zero_for_one: true,
                volume_usd_e8: 10_000e8,
                fee_bps: 30,
                liquidity: 1_000_000,
                amount0_delta: -100,
                amount1_delta: 475
            })
        );
        _assertEqBytes4(after_selector, NAVGuardHook.afterSwap.selector, "after selector");

        NAVGuardHook.PoolState memory state = hook.poolState(POOL_ID);
        _assertEqUint(state.swap_count, 1, "swap count");
        _assertEqUint(state.depth_count, 0, "depth count");
        _assertEqUint(state.checkpoint_count, 1, "checkpoint count");
        _assertTrue(state.swap_root != bytes32(0), "swap root");

        NAVGuardHook.SwapObservation memory observation = hook.latestSwapObservation(POOL_ID);
        _assertEqUint64(observation.sequence, 1, "sequence");
        _assertEqBytes32(observation.pool_id, POOL_ID, "pool id");
        _assertEqUint(observation.price_usd_e8, 475_000_000, "price");
        _assertTrue(observation.zero_for_one, "direction");
        _assertEqUint(observation.volume_usd_e8, 10_000e8, "volume");
        _assertEqUint32(observation.fee_bps, 30, "fee");
        _assertEqUint128(observation.liquidity, 1_000_000, "liquidity");
        _assertEqInt(observation.amount0_delta, -100, "amount0");
        _assertEqInt(observation.amount1_delta, 475, "amount1");
        _assertEqBytes32(observation.pftl_state_hash, PFTL_STATE_HASH, "pftl state");
        _assertTrue(observation.observation_hash != bytes32(0), "observation hash");
    }

    function testLiquidityChangeProducesDepthCheckpoint() public {
        bytes4 add_selector = manager.afterAddLiquidity(
            NAVGuardHook.DepthObservationInput({
                pool_id: POOL_ID, liquidity_delta: 1_000, liquidity_after: 101_000, depth_usd_e8: 500_000e8
            })
        );
        _assertEqBytes4(add_selector, NAVGuardHook.afterAddLiquidity.selector, "add selector");

        NAVGuardHook.PoolState memory state = hook.poolState(POOL_ID);
        _assertEqUint(state.swap_count, 0, "swap count");
        _assertEqUint(state.depth_count, 1, "depth count");
        _assertEqUint(state.checkpoint_count, 1, "checkpoint count");
        _assertTrue(state.depth_root != bytes32(0), "depth root");

        NAVGuardHook.DepthObservation memory observation = hook.latestDepthObservation(POOL_ID);
        _assertEqUint64(observation.sequence, 1, "sequence");
        _assertEqBytes32(observation.pool_id, POOL_ID, "pool id");
        _assertTrue(observation.added, "added");
        _assertEqUint128(observation.liquidity_delta, 1_000, "delta");
        _assertEqUint128(observation.liquidity_after, 101_000, "liquidity after");
        _assertEqUint(observation.depth_usd_e8, 500_000e8, "depth");
        _assertEqBytes32(observation.pftl_state_hash, PFTL_STATE_HASH, "pftl state");
        _assertTrue(observation.observation_hash != bytes32(0), "observation hash");

        bytes4 remove_selector = manager.afterRemoveLiquidity(
            NAVGuardHook.DepthObservationInput({
                pool_id: POOL_ID, liquidity_delta: 250, liquidity_after: 100_750, depth_usd_e8: 498_000e8
            })
        );
        _assertEqBytes4(remove_selector, NAVGuardHook.afterRemoveLiquidity.selector, "remove selector");
        observation = hook.latestDepthObservation(POOL_ID);
        _assertTrue(!observation.added, "removed");
        _assertEqUint64(observation.sequence, 2, "remove sequence");
    }

    function testStaleStateRejectionWorks() public {
        vm.warp(1_301);
        _assertTrue(!hook.isPFTLStateFresh(POOL_ID), "stale state");

        _expectBeforeSwapRevert(POOL_ID);
        _expectAfterSwapRevert(
            NAVGuardHook.SwapObservationInput({
                pool_id: POOL_ID,
                price_usd_e8: 475_000_000,
                zero_for_one: true,
                volume_usd_e8: 10_000e8,
                fee_bps: 30,
                liquidity: 1_000_000,
                amount0_delta: -100,
                amount1_delta: 475
            })
        );
    }

    function _expectBeforeSwapRevert(bytes32 pool_id) private view {
        try manager.beforeSwap(pool_id) returns (bytes4) {
            revert("expected beforeSwap revert");
        } catch {}
    }

    function _expectAfterSwapRevert(NAVGuardHook.SwapObservationInput memory input) private {
        try manager.afterSwap(input) returns (bytes4) {
            revert("expected afterSwap revert");
        } catch {}
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) {
            revert(message);
        }
    }

    function _assertEqBytes4(bytes4 actual, bytes4 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEqBytes32(bytes32 actual, bytes32 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEqUint64(uint64 actual, uint64 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEqUint32(uint32 actual, uint32 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEqUint128(uint128 actual, uint128 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEqUint(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEqInt(int256 actual, int256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}

contract MockPoolManager {
    NAVGuardHook private hook;

    function setHook(NAVGuardHook hook_) external {
        hook = hook_;
    }

    function beforeSwap(bytes32 pool_id) external view returns (bytes4) {
        return hook.beforeSwap(pool_id);
    }

    function afterSwap(NAVGuardHook.SwapObservationInput memory input) external returns (bytes4) {
        return hook.afterSwap(input);
    }

    function afterAddLiquidity(NAVGuardHook.DepthObservationInput memory input) external returns (bytes4) {
        return hook.afterAddLiquidity(input);
    }

    function afterRemoveLiquidity(NAVGuardHook.DepthObservationInput memory input) external returns (bytes4) {
        return hook.afterRemoveLiquidity(input);
    }
}
