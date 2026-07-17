// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @notice Dependency-light Uniswap v4-shaped hook for NAVCoin venue evidence.
/// @dev The callable entry points mirror v4 hook phases but avoid external v4 package
///      dependencies in this repository. A real pool manager adapter can map v4 PoolKey,
///      SwapParams, BalanceDelta, and liquidity callbacks into these replayable records.
contract NAVGuardHook {
    struct SwapObservationInput {
        bytes32 pool_id;
        uint256 price_usd_e8;
        bool zero_for_one;
        uint256 volume_usd_e8;
        uint32 fee_bps;
        uint128 liquidity;
        int256 amount0_delta;
        int256 amount1_delta;
    }

    struct DepthObservationInput {
        bytes32 pool_id;
        uint128 liquidity_delta;
        uint128 liquidity_after;
        uint256 depth_usd_e8;
    }

    struct SwapObservation {
        uint64 timestamp;
        uint64 block_number;
        uint64 sequence;
        bytes32 pool_id;
        uint256 price_usd_e8;
        bool zero_for_one;
        uint256 volume_usd_e8;
        uint32 fee_bps;
        uint128 liquidity;
        int256 amount0_delta;
        int256 amount1_delta;
        bytes32 pftl_state_hash;
        bytes32 observation_hash;
    }

    struct DepthObservation {
        uint64 timestamp;
        uint64 block_number;
        uint64 sequence;
        bytes32 pool_id;
        bool added;
        uint128 liquidity_delta;
        uint128 liquidity_after;
        uint256 depth_usd_e8;
        bytes32 pftl_state_hash;
        bytes32 observation_hash;
    }

    struct PoolState {
        bool registered;
        bytes32 pool_config_hash;
        bytes32 pftl_state_hash;
        uint64 pftl_state_updated_at;
        uint256 swap_count;
        uint256 depth_count;
        uint256 checkpoint_count;
        bytes32 swap_root;
        bytes32 depth_root;
    }

    error NotOwner();
    error NotPoolManager();
    error ZeroOwner();
    error ZeroAddress(bytes32 field);
    error ZeroField(bytes32 field);
    error InvalidCapacity();
    error InvalidCheckpointInterval();
    error PoolAlreadyRegistered(bytes32 pool_id);
    error PoolNotRegistered(bytes32 pool_id);
    error InvalidObservation(bytes32 field);
    error StalePFTLState(bytes32 pool_id, uint64 now_timestamp, uint64 stale_after);
    error TimestampOverflow(uint256 value);
    error BlockNumberOverflow(uint256 value);

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event PoolRegistered(bytes32 indexed pool_id, bytes32 pool_config_hash, address indexed hook);
    event PFTLStateUpdated(bytes32 indexed pool_id, bytes32 pftl_state_hash, uint64 updated_at);
    event SwapObservationRecorded(
        bytes32 indexed pool_id,
        uint64 indexed sequence,
        bytes32 indexed observation_hash,
        uint256 price_usd_e8,
        bool zero_for_one,
        uint256 volume_usd_e8,
        uint32 fee_bps,
        uint128 liquidity,
        bytes32 pftl_state_hash
    );
    event DepthObservationRecorded(
        bytes32 indexed pool_id,
        uint64 indexed sequence,
        bytes32 indexed observation_hash,
        bool added,
        uint128 liquidity_delta,
        uint128 liquidity_after,
        uint256 depth_usd_e8,
        bytes32 pftl_state_hash
    );
    event ObservationCheckpoint(
        bytes32 indexed pool_id,
        uint256 indexed checkpoint_count,
        uint256 swap_count,
        uint256 depth_count,
        bytes32 swap_root,
        bytes32 depth_root,
        bytes32 pftl_state_hash
    );

    address public immutable pool_manager;
    uint256 public immutable swap_observation_capacity;
    uint256 public immutable depth_observation_capacity;
    uint256 public immutable checkpoint_interval;
    uint64 public immutable max_pftl_staleness_seconds;

    address public owner;

    mapping(bytes32 => PoolState) private pools;
    mapping(bytes32 => mapping(uint256 => SwapObservation)) private swap_observations;
    mapping(bytes32 => mapping(uint256 => DepthObservation)) private depth_observations;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier onlyPoolManager() {
        if (msg.sender != pool_manager) {
            revert NotPoolManager();
        }
        _;
    }

    constructor(
        address pool_manager_,
        address initial_owner,
        uint256 swap_capacity,
        uint256 depth_capacity,
        uint256 checkpoint_interval_,
        uint64 max_pftl_staleness_seconds_
    ) {
        if (pool_manager_ == address(0)) {
            revert ZeroAddress("pool_manager");
        }
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        if (swap_capacity == 0 || depth_capacity == 0) {
            revert InvalidCapacity();
        }
        if (checkpoint_interval_ == 0) {
            revert InvalidCheckpointInterval();
        }

        pool_manager = pool_manager_;
        owner = initial_owner;
        swap_observation_capacity = swap_capacity;
        depth_observation_capacity = depth_capacity;
        checkpoint_interval = checkpoint_interval_;
        max_pftl_staleness_seconds = max_pftl_staleness_seconds_;

        emit OwnershipTransferred(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function registerPool(bytes32 pool_id, bytes32 pool_config_hash) external onlyOwner {
        if (pool_id == bytes32(0)) {
            revert ZeroField("pool_id");
        }
        if (pool_config_hash == bytes32(0)) {
            revert ZeroField("pool_config_hash");
        }
        PoolState storage pool = pools[pool_id];
        if (pool.registered) {
            revert PoolAlreadyRegistered(pool_id);
        }

        pool.registered = true;
        pool.pool_config_hash = pool_config_hash;
        emit PoolRegistered(pool_id, pool_config_hash, address(this));
    }

    function updatePFTLState(bytes32 pool_id, bytes32 pftl_state_hash) external onlyOwner {
        if (pftl_state_hash == bytes32(0)) {
            revert ZeroField("pftl_state_hash");
        }
        PoolState storage pool = _registeredPool(pool_id);
        uint64 now_timestamp = _now64();
        pool.pftl_state_hash = pftl_state_hash;
        pool.pftl_state_updated_at = now_timestamp;
        emit PFTLStateUpdated(pool_id, pftl_state_hash, now_timestamp);
    }

    function beforeSwap(bytes32 pool_id) external view onlyPoolManager returns (bytes4) {
        _requireFreshPFTLState(pool_id);
        return NAVGuardHook.beforeSwap.selector;
    }

    function afterSwap(SwapObservationInput calldata input) external onlyPoolManager returns (bytes4) {
        if (input.price_usd_e8 == 0) {
            revert InvalidObservation("price_usd_e8");
        }
        _requireFreshPFTLState(input.pool_id);
        PoolState storage pool = pools[input.pool_id];

        uint64 sequence = _nextSequence(pool.swap_count);
        bytes32 observation_hash = _swapObservationHash(input, sequence, pool.pftl_state_hash);
        SwapObservation memory observation = SwapObservation({
            timestamp: _now64(),
            block_number: _blockNumber64(),
            sequence: sequence,
            pool_id: input.pool_id,
            price_usd_e8: input.price_usd_e8,
            zero_for_one: input.zero_for_one,
            volume_usd_e8: input.volume_usd_e8,
            fee_bps: input.fee_bps,
            liquidity: input.liquidity,
            amount0_delta: input.amount0_delta,
            amount1_delta: input.amount1_delta,
            pftl_state_hash: pool.pftl_state_hash,
            observation_hash: observation_hash
        });

        swap_observations[input.pool_id][pool.swap_count % swap_observation_capacity] = observation;
        pool.swap_count += 1;
        pool.swap_root = keccak256(abi.encode(pool.swap_root, observation_hash));

        emit SwapObservationRecorded(
            input.pool_id,
            sequence,
            observation_hash,
            input.price_usd_e8,
            input.zero_for_one,
            input.volume_usd_e8,
            input.fee_bps,
            input.liquidity,
            pool.pftl_state_hash
        );
        _checkpointIfDue(input.pool_id, pool);
        return NAVGuardHook.afterSwap.selector;
    }

    function afterAddLiquidity(DepthObservationInput calldata input) external onlyPoolManager returns (bytes4) {
        _recordDepth(input, true);
        return NAVGuardHook.afterAddLiquidity.selector;
    }

    function afterRemoveLiquidity(DepthObservationInput calldata input) external onlyPoolManager returns (bytes4) {
        _recordDepth(input, false);
        return NAVGuardHook.afterRemoveLiquidity.selector;
    }

    function checkpoint(bytes32 pool_id) external {
        PoolState storage pool = _registeredPool(pool_id);
        _emitCheckpoint(pool_id, pool);
    }

    function poolState(bytes32 pool_id) external view returns (PoolState memory) {
        return pools[pool_id];
    }

    function swapObservationAt(bytes32 pool_id, uint256 ring_index) external view returns (SwapObservation memory) {
        return swap_observations[pool_id][ring_index];
    }

    function depthObservationAt(bytes32 pool_id, uint256 ring_index) external view returns (DepthObservation memory) {
        return depth_observations[pool_id][ring_index];
    }

    function latestSwapObservation(bytes32 pool_id) external view returns (SwapObservation memory) {
        PoolState storage pool = pools[pool_id];
        if (pool.swap_count == 0) {
            revert InvalidObservation("swap_count");
        }
        return swap_observations[pool_id][(pool.swap_count - 1) % swap_observation_capacity];
    }

    function latestDepthObservation(bytes32 pool_id) external view returns (DepthObservation memory) {
        PoolState storage pool = pools[pool_id];
        if (pool.depth_count == 0) {
            revert InvalidObservation("depth_count");
        }
        return depth_observations[pool_id][(pool.depth_count - 1) % depth_observation_capacity];
    }

    function isPFTLStateFresh(bytes32 pool_id) public view returns (bool) {
        PoolState storage pool = pools[pool_id];
        if (!pool.registered || pool.pftl_state_hash == bytes32(0)) {
            return false;
        }
        uint64 now_timestamp = _now64();
        uint64 stale_after = pool.pftl_state_updated_at + max_pftl_staleness_seconds;
        return now_timestamp <= stale_after;
    }

    function _recordDepth(DepthObservationInput calldata input, bool added) private {
        if (input.liquidity_delta == 0) {
            revert InvalidObservation("liquidity_delta");
        }
        _requireFreshPFTLState(input.pool_id);
        PoolState storage pool = pools[input.pool_id];

        uint64 sequence = _nextSequence(pool.depth_count);
        bytes32 observation_hash = _depthObservationHash(input, added, sequence, pool.pftl_state_hash);
        DepthObservation memory observation = DepthObservation({
            timestamp: _now64(),
            block_number: _blockNumber64(),
            sequence: sequence,
            pool_id: input.pool_id,
            added: added,
            liquidity_delta: input.liquidity_delta,
            liquidity_after: input.liquidity_after,
            depth_usd_e8: input.depth_usd_e8,
            pftl_state_hash: pool.pftl_state_hash,
            observation_hash: observation_hash
        });

        depth_observations[input.pool_id][pool.depth_count % depth_observation_capacity] = observation;
        pool.depth_count += 1;
        pool.depth_root = keccak256(abi.encode(pool.depth_root, observation_hash));

        emit DepthObservationRecorded(
            input.pool_id,
            sequence,
            observation_hash,
            added,
            input.liquidity_delta,
            input.liquidity_after,
            input.depth_usd_e8,
            pool.pftl_state_hash
        );
        _checkpointIfDue(input.pool_id, pool);
    }

    function _checkpointIfDue(bytes32 pool_id, PoolState storage pool) private {
        uint256 observation_count = pool.swap_count + pool.depth_count;
        if (observation_count % checkpoint_interval == 0) {
            _emitCheckpoint(pool_id, pool);
        }
    }

    function _emitCheckpoint(bytes32 pool_id, PoolState storage pool) private {
        pool.checkpoint_count += 1;
        emit ObservationCheckpoint(
            pool_id,
            pool.checkpoint_count,
            pool.swap_count,
            pool.depth_count,
            pool.swap_root,
            pool.depth_root,
            pool.pftl_state_hash
        );
    }

    function _registeredPool(bytes32 pool_id) private view returns (PoolState storage pool) {
        pool = pools[pool_id];
        if (!pool.registered) {
            revert PoolNotRegistered(pool_id);
        }
    }

    function _requireFreshPFTLState(bytes32 pool_id) private view {
        PoolState storage pool = _registeredPool(pool_id);
        if (pool.pftl_state_hash == bytes32(0)) {
            revert StalePFTLState(pool_id, _now64(), 0);
        }
        uint64 stale_after = pool.pftl_state_updated_at + max_pftl_staleness_seconds;
        uint64 now_timestamp = _now64();
        if (now_timestamp > stale_after) {
            revert StalePFTLState(pool_id, now_timestamp, stale_after);
        }
    }

    function _swapObservationHash(SwapObservationInput calldata input, uint64 sequence, bytes32 pftl_state_hash)
        private
        view
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                "navguard.swap.v1",
                input.pool_id,
                pools[input.pool_id].pool_config_hash,
                sequence,
                block.chainid,
                block.number,
                block.timestamp,
                input.price_usd_e8,
                input.zero_for_one,
                input.volume_usd_e8,
                input.fee_bps,
                input.liquidity,
                input.amount0_delta,
                input.amount1_delta,
                pftl_state_hash
            )
        );
    }

    function _depthObservationHash(
        DepthObservationInput calldata input,
        bool added,
        uint64 sequence,
        bytes32 pftl_state_hash
    ) private view returns (bytes32) {
        return keccak256(
            abi.encode(
                "navguard.depth.v1",
                input.pool_id,
                pools[input.pool_id].pool_config_hash,
                sequence,
                block.chainid,
                block.number,
                block.timestamp,
                added,
                input.liquidity_delta,
                input.liquidity_after,
                input.depth_usd_e8,
                pftl_state_hash
            )
        );
    }

    function _nextSequence(uint256 current_count) private pure returns (uint64) {
        uint256 sequence = current_count + 1;
        if (sequence > type(uint64).max) {
            revert InvalidObservation("sequence");
        }
        // casting to uint64 is safe because the guard above rejects larger sequences.
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint64(sequence);
    }

    function _now64() private view returns (uint64) {
        uint256 timestamp = block.timestamp;
        if (timestamp > type(uint64).max) {
            revert TimestampOverflow(timestamp);
        }
        // casting to uint64 is safe because the guard above rejects larger timestamps.
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint64(timestamp);
    }

    function _blockNumber64() private view returns (uint64) {
        if (block.number > type(uint64).max) {
            revert BlockNumberOverflow(block.number);
        }
        // casting to uint64 is safe because the guard above rejects larger block numbers.
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint64(block.number);
    }
}
