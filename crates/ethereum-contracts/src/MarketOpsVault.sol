// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "./MarketOpsEnvelope.sol";

interface IERC20Minimal {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

interface IBurnableToken is IERC20Minimal {
    function burn(uint256 amount) external;
}

interface IPFTLBridgeAdapter {
    function getEvmEnvelopeDigest(bytes32 pending_id) external view returns (bytes32);
    function isEnvelopeExecutable(bytes32 pending_id) external view returns (bool);
    function reserveDeployCapUsdE8(bytes32 pending_id) external view returns (uint256);
}

interface IMarketOpsBuyVenue {
    function executeBuy(
        address reserve_token,
        address asset_token,
        uint256 reserve_amount,
        uint256 min_received,
        address recipient,
        bytes calldata data
    ) external returns (uint256 reported_received);
}

/// @notice Custodies venue alignment reserves and executes bounded below-NAV buys.
contract MarketOpsVault {
    struct BuyRoute {
        bytes32 pending_id;
        address venue;
        bytes data;
    }

    error NotOwner();
    error ZeroOwner();
    error ZeroAddress(bytes32 field);
    error AdapterAlreadySet(address adapter);
    error AdapterUnset();
    error InvalidBps(uint256 bps);
    error InvalidAmount();
    error DeadlineExpired(uint256 now_timestamp, uint256 deadline);
    error VenueNotConfigured(bytes32 venue_id);
    error RouteVenueMismatch(bytes32 venue_id, address expected, address actual);
    error EnvelopeDigestMismatch(bytes32 expected, bytes32 actual);
    error EnvelopeNotExecutable(bytes32 pending_id);
    error CapExceeded(bytes32 pending_id, uint256 attempted_usd_e8, uint256 cap_usd_e8);
    error VenueReserveInsufficient(bytes32 venue_id, uint256 requested, uint256 available);
    error CooldownActive(bytes32 pending_id, uint256 now_timestamp, uint256 next_allowed);
    error ReceivedBelowMinimum(uint256 received, uint256 min_received);
    error PriceAboveLimit(uint256 execution_price_usd_e8, uint256 max_buy_price_usd_e8);
    error ReserveTransferFailed();
    error ReserveTransferFromFailed();

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event BridgeAdapterSet(address indexed adapter);
    event VenueConfigured(bytes32 indexed venue_id, address indexed venue);
    event VenueReserveFunded(bytes32 indexed venue_id, address indexed funder, uint256 amount);
    event BuyExecuted(
        bytes32 indexed pending_id,
        bytes32 indexed asset_id,
        uint64 indexed epoch,
        bytes32 venue_id,
        address venue,
        uint256 reserve_amount,
        uint256 reserve_amount_usd_e8,
        uint256 received_atoms,
        uint256 execution_price_usd_e8,
        uint256 max_buy_price_usd_e8,
        bool burned
    );
    event TreasuryInventoryLocked(bytes32 indexed asset_id, uint256 amount);
    event TreasuryInventoryBurned(bytes32 indexed asset_id, uint256 amount);

    uint256 public constant BPS = 10_000;
    uint256 public constant RESERVE_TOKEN_TO_USD_E8 = 100;

    IERC20Minimal public immutable reserve_token;
    IBurnableToken public immutable asset_token;
    bool public immutable burn_acquired;
    uint256 public immutable support_discount_bps;

    address public owner;
    IPFTLBridgeAdapter public bridge_adapter;

    mapping(bytes32 => address) public venue_by_id;
    mapping(bytes32 => uint256) public venue_reserve_balance;
    mapping(bytes32 => uint256) public deployed_usd_e8_by_pending_id;
    mapping(bytes32 => uint256) public last_execution_at_by_pending_id;
    mapping(bytes32 => uint256) public locked_treasury_inventory_atoms_by_asset;
    mapping(bytes32 => uint256) public burned_atoms_by_asset;

    uint256 private reentrancy_lock;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier nonReentrant() {
        if (reentrancy_lock != 0) {
            revert("reentrant");
        }
        reentrancy_lock = 1;
        _;
        reentrancy_lock = 0;
    }

    constructor(
        IERC20Minimal reserve_token_,
        IBurnableToken asset_token_,
        address initial_owner,
        uint256 support_discount_bps_,
        bool burn_acquired_
    ) {
        if (address(reserve_token_) == address(0)) {
            revert ZeroAddress("reserve_token");
        }
        if (address(asset_token_) == address(0)) {
            revert ZeroAddress("asset_token");
        }
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        if (support_discount_bps_ > BPS) {
            revert InvalidBps(support_discount_bps_);
        }

        reserve_token = reserve_token_;
        asset_token = asset_token_;
        owner = initial_owner;
        support_discount_bps = support_discount_bps_;
        burn_acquired = burn_acquired_;

        emit OwnershipTransferred(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setBridgeAdapter(IPFTLBridgeAdapter adapter) external onlyOwner {
        if (address(adapter) == address(0)) {
            revert ZeroAddress("bridge_adapter");
        }
        if (address(bridge_adapter) != address(0)) {
            revert AdapterAlreadySet(address(bridge_adapter));
        }
        bridge_adapter = adapter;
        emit BridgeAdapterSet(address(adapter));
    }

    function setVenue(bytes32 venue_id, address venue) external onlyOwner {
        if (venue_id == bytes32(0)) {
            revert ZeroAddress("venue_id");
        }
        if (venue == address(0)) {
            revert ZeroAddress("venue");
        }
        venue_by_id[venue_id] = venue;
        emit VenueConfigured(venue_id, venue);
    }

    function fundVenueReserve(bytes32 venue_id, uint256 amount) external nonReentrant {
        if (amount == 0) {
            revert InvalidAmount();
        }
        if (venue_by_id[venue_id] == address(0)) {
            revert VenueNotConfigured(venue_id);
        }

        _safeTransferFrom(reserve_token, msg.sender, address(this), amount);
        venue_reserve_balance[venue_id] += amount;
        emit VenueReserveFunded(venue_id, msg.sender, amount);
    }

    function executeBuy(
        MarketOpsEnvelope calldata envelope,
        BuyRoute calldata route,
        uint256 amount,
        uint256 min_received,
        uint256 deadline
    ) external nonReentrant returns (uint256 received) {
        if (address(bridge_adapter) == address(0)) {
            revert AdapterUnset();
        }
        if (amount == 0 || min_received == 0) {
            revert InvalidAmount();
        }
        _validateExecutionEnvelope(envelope, route, deadline);

        uint256 amount_usd_e8 =
            _reserveAndAccount(envelope.venue_id, route.pending_id, amount, envelope.cooldown_seconds);
        received = _executeVenue(route, amount, min_received);
        uint256 max_buy_price_usd_e8 = maxBuyPriceUsdE8(envelope.nav_floor_usd_e8);
        uint256 execution_price_usd_e8 = _ceilDiv(amount_usd_e8, received);
        if (execution_price_usd_e8 > max_buy_price_usd_e8) {
            revert PriceAboveLimit(execution_price_usd_e8, max_buy_price_usd_e8);
        }

        _settleAcquired(envelope.asset_id, received);

        emit BuyExecuted(
            route.pending_id,
            envelope.asset_id,
            envelope.epoch,
            envelope.venue_id,
            route.venue,
            amount,
            amount_usd_e8,
            received,
            execution_price_usd_e8,
            max_buy_price_usd_e8,
            burn_acquired
        );
    }

    function maxBuyPriceUsdE8(uint256 nav_floor_usd_e8) public view returns (uint256) {
        return nav_floor_usd_e8 * (BPS - support_discount_bps) / BPS;
    }

    function _validateExecutionEnvelope(MarketOpsEnvelope calldata envelope, BuyRoute calldata route, uint256 deadline)
        private
        view
    {
        uint256 now_timestamp = block.timestamp;
        if (now_timestamp > deadline) {
            revert DeadlineExpired(now_timestamp, deadline);
        }
        address expected_venue = venue_by_id[envelope.venue_id];
        if (expected_venue == address(0)) {
            revert VenueNotConfigured(envelope.venue_id);
        }
        if (route.venue != expected_venue) {
            revert RouteVenueMismatch(envelope.venue_id, expected_venue, route.venue);
        }

        bytes32 expected_digest = bridge_adapter.getEvmEnvelopeDigest(route.pending_id);
        bytes32 actual_digest = keccak256(abi.encode(envelope));
        if (expected_digest != actual_digest) {
            revert EnvelopeDigestMismatch(expected_digest, actual_digest);
        }
        if (!bridge_adapter.isEnvelopeExecutable(route.pending_id)) {
            revert EnvelopeNotExecutable(route.pending_id);
        }
    }

    function _reserveAndAccount(bytes32 venue_id, bytes32 pending_id, uint256 amount, uint64 cooldown_seconds)
        private
        returns (uint256 amount_usd_e8)
    {
        amount_usd_e8 = amount * RESERVE_TOKEN_TO_USD_E8;
        uint256 new_deployed_usd_e8 = deployed_usd_e8_by_pending_id[pending_id] + amount_usd_e8;
        uint256 cap_usd_e8 = bridge_adapter.reserveDeployCapUsdE8(pending_id);
        if (new_deployed_usd_e8 > cap_usd_e8) {
            revert CapExceeded(pending_id, new_deployed_usd_e8, cap_usd_e8);
        }

        uint256 venue_reserve = venue_reserve_balance[venue_id];
        if (amount > venue_reserve) {
            revert VenueReserveInsufficient(venue_id, amount, venue_reserve);
        }

        uint256 last_execution_at = last_execution_at_by_pending_id[pending_id];
        if (last_execution_at != 0) {
            uint256 next_allowed = last_execution_at + cooldown_seconds;
            uint256 now_timestamp = block.timestamp;
            if (now_timestamp < next_allowed) {
                revert CooldownActive(pending_id, now_timestamp, next_allowed);
            }
        }

        venue_reserve_balance[venue_id] = venue_reserve - amount;
        deployed_usd_e8_by_pending_id[pending_id] = new_deployed_usd_e8;
        last_execution_at_by_pending_id[pending_id] = block.timestamp;
    }

    function _executeVenue(BuyRoute calldata route, uint256 amount, uint256 min_received)
        private
        returns (uint256 received)
    {
        uint256 asset_balance_before = asset_token.balanceOf(address(this));
        _safeTransfer(reserve_token, route.venue, amount);
        IMarketOpsBuyVenue(route.venue)
            .executeBuy(address(reserve_token), address(asset_token), amount, min_received, address(this), route.data);

        received = asset_token.balanceOf(address(this)) - asset_balance_before;
        if (received < min_received) {
            revert ReceivedBelowMinimum(received, min_received);
        }
    }

    function _settleAcquired(bytes32 asset_id, uint256 received) private {
        if (burn_acquired) {
            asset_token.burn(received);
            burned_atoms_by_asset[asset_id] += received;
            emit TreasuryInventoryBurned(asset_id, received);
        } else {
            locked_treasury_inventory_atoms_by_asset[asset_id] += received;
            emit TreasuryInventoryLocked(asset_id, received);
        }
    }

    function _ceilDiv(uint256 numerator, uint256 denominator) private pure returns (uint256) {
        return numerator == 0 ? 0 : (numerator - 1) / denominator + 1;
    }

    function _safeTransfer(IERC20Minimal token, address to, uint256 amount) private {
        bool ok = token.transfer(to, amount);
        if (!ok) {
            revert ReserveTransferFailed();
        }
    }

    function _safeTransferFrom(IERC20Minimal token, address from, address to, uint256 amount) private {
        bool ok = token.transferFrom(from, to, amount);
        if (!ok) {
            revert ReserveTransferFromFailed();
        }
    }
}
