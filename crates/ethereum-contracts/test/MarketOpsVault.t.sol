// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "../src/MarketOpsEnvelope.sol";
import {IBurnableToken, IERC20Minimal, IPFTLBridgeAdapter, MarketOpsVault} from "../src/MarketOpsVault.sol";
import {PFTLBridgeAdapter} from "../src/PFTLBridgeAdapter.sol";
import {PolicyRegistry} from "../src/PolicyRegistry.sol";

interface VaultVm {
    function warp(uint256 timestamp) external;
}

contract MarketOpsVaultTest {
    VaultVm private constant vm = VaultVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    MockToken private reserve_token;
    MockToken private asset;
    MockVenue private venue;
    PolicyRegistry private registry;
    PFTLBridgeAdapter private adapter;
    MarketOpsVault private vault;

    uint64 private constant CHAIN_ID = 65_100;
    uint64 private constant CHALLENGE_DELAY = 100;
    uint64 private constant EXECUTION_WINDOW = 1_000;
    uint64 private constant MAX_STALENESS = 75;
    uint256 private constant SUPPORT_DISCOUNT_BPS = 500;

    address private constant MINT_CONTROLLER = address(0x1313);

    bytes32 private constant ASSET_ID = bytes32(uint256(0xaaaaaaaa));
    bytes32 private constant PROGRAM_ID = bytes32(uint256(0x31));
    bytes32 private constant POLICY_HASH = bytes32(uint256(0x32));
    bytes32 private constant PARAMETER_HASH = bytes32(uint256(0x33));
    bytes32 private constant VENUE_ID = bytes32(uint256(0x37));
    bytes32 private constant POOL_CONFIG_HASH = bytes32(uint256(0x38));
    bytes32 private constant HOOK_CODE_HASH = bytes32(uint256(0x39));

    bytes private constant ENVELOPE_HASH =
        hex"0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30";

    function setUp() public {
        reserve_token = new MockToken();
        asset = new MockToken();
        venue = new MockVenue(asset);

        registry = new PolicyRegistry(address(this));
        registry.registerPolicy(
            PROGRAM_ID, POLICY_HASH, PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 1, 0
        );

        vault = new MarketOpsVault(
            IERC20Minimal(address(reserve_token)),
            IBurnableToken(address(asset)),
            address(this),
            SUPPORT_DISCOUNT_BPS,
            true
        );
        adapter = new PFTLBridgeAdapter(
            registry,
            address(this),
            CHAIN_ID,
            address(vault),
            MINT_CONTROLLER,
            CHALLENGE_DELAY,
            EXECUTION_WINDOW,
            MAX_STALENESS
        );
        vault.setBridgeAdapter(IPFTLBridgeAdapter(address(adapter)));
        vault.setVenue(VENUE_ID, address(venue));
    }

    function testVaultBuysBelowPriceLimit() public {
        venue.setPriceUsdE8(470_000_000);
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 600);
        _fund(47_000_000);

        uint256 received = vault.executeBuy(envelope, _route(pending_id), 47_000_000, 10, 1_200);

        _assertEq(received, 10, "received");
        _assertEq(reserve_token.balanceOf(address(venue)), 47_000_000, "venue reserve");
        _assertEq(vault.deployed_usd_e8_by_pending_id(pending_id), 47e8, "deployed");
        _assertEq(vault.venue_reserve_balance(VENUE_ID), 0, "venue reserve");
    }

    function testVaultRevertsAbovePriceLimit() public {
        venue.setPriceUsdE8(480_000_000);
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 600);
        _fund(48_000_000);

        _expectExecuteRevert(envelope, _route(pending_id), 48_000_000, 10, 1_200);

        _assertEq(vault.deployed_usd_e8_by_pending_id(pending_id), 0, "deployed remains zero");
        _assertEq(vault.venue_reserve_balance(VENUE_ID), 48_000_000, "reserve remains");
        _assertEq(asset.totalSupply(), 0, "asset supply remains");
    }

    function testVaultRespectsEnvelopeCap() public {
        venue.setPriceUsdE8(470_000_000);
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 50e8, 0);
        _fund(60_000_000);

        uint256 first_received = vault.executeBuy(envelope, _route(pending_id), 47_000_000, 10, 1_200);
        _assertEq(first_received, 10, "first received");

        _expectExecuteRevert(envelope, _route(pending_id), 4_700_000, 1, 1_200);

        _assertEq(vault.deployed_usd_e8_by_pending_id(pending_id), 47e8, "cap accounting unchanged");
        _assertEq(vault.venue_reserve_balance(VENUE_ID), 13_000_000, "remaining reserve");
    }

    function testVaultBurnsAcquiredUnits() public {
        venue.setPriceUsdE8(470_000_000);
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 600);
        _fund(47_000_000);

        uint256 received = vault.executeBuy(envelope, _route(pending_id), 47_000_000, 10, 1_200);

        _assertEq(received, 10, "received");
        _assertEq(asset.balanceOf(address(vault)), 0, "vault asset balance");
        _assertEq(asset.totalSupply(), 0, "burned total supply");
        _assertEq(vault.burned_atoms_by_asset(ASSET_ID), 10, "burned accounting");
        _assertEq(vault.locked_treasury_inventory_atoms_by_asset(ASSET_ID), 0, "locked inventory");
    }

    function _acceptedEnvelope(uint64 epoch, uint256 max_reserve_deploy_usd_e8, uint64 cooldown_seconds)
        private
        returns (MarketOpsEnvelope memory envelope, bytes32 pending_id)
    {
        vm.warp(1_000);
        envelope = _envelope(epoch, 1_100, 1_600, 950, max_reserve_deploy_usd_e8, cooldown_seconds);
        pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);
        vm.warp(1_100);
        adapter.finalizeEnvelope(pending_id);
    }

    function _envelope(
        uint64 epoch,
        uint64 valid_after,
        uint64 expires_at,
        uint64 data_window_end,
        uint256 max_reserve_deploy_usd_e8,
        uint64 cooldown_seconds
    ) private view returns (MarketOpsEnvelope memory envelope) {
        envelope.encoding_version = 1;
        envelope.chain_id = CHAIN_ID;
        envelope.adapter_address = address(adapter);
        envelope.vault_address = address(vault);
        envelope.mint_controller_address = MINT_CONTROLLER;
        envelope.asset_id = ASSET_ID;
        envelope.epoch = epoch;
        envelope.program_id = PROGRAM_ID;
        envelope.policy_hash = POLICY_HASH;
        envelope.parameter_hash = PARAMETER_HASH;
        envelope.reserve_packet_hash = bytes32(uint256(0xabab));
        envelope.supply_packet_hash = bytes32(uint256(0xcdcd));
        envelope.evidence_root = bytes32(uint256(0xefef));
        envelope.previous_market_state_hash = bytes32(0);
        envelope.venue_id = VENUE_ID;
        envelope.pool_config_hash = POOL_CONFIG_HASH;
        envelope.hook_code_hash = HOOK_CODE_HASH;
        envelope.nav_floor_usd_e8 = 5e8;
        envelope.valid_global_supply_atoms = 1_000_000;
        envelope.verified_net_assets_usd_e8 = 5_000_000e8;
        envelope.funded_alignment_reserve_usd_e8 = 150_000e8;
        envelope.required_alignment_reserve_usd_e8 = 135_000e8;
        envelope.max_reserve_deploy_usd_e8 = max_reserve_deploy_usd_e8;
        envelope.max_mint_atoms = 0;
        envelope.discount_trigger_bps = 300;
        envelope.premium_trigger_bps = 1_000;
        envelope.data_window_start = data_window_end - 100;
        envelope.data_window_end = data_window_end;
        envelope.valid_after = valid_after;
        envelope.expires_at = expires_at;
        envelope.cooldown_seconds = cooldown_seconds;
        envelope.nonce = bytes32(uint256(0x55));
    }

    function _route(bytes32 pending_id) private view returns (MarketOpsVault.BuyRoute memory) {
        return MarketOpsVault.BuyRoute({pending_id: pending_id, venue: address(venue), data: ""});
    }

    function _fund(uint256 amount) private {
        reserve_token.mint(address(this), amount);
        reserve_token.approve(address(vault), amount);
        vault.fundVenueReserve(VENUE_ID, amount);
    }

    function _expectExecuteRevert(
        MarketOpsEnvelope memory envelope,
        MarketOpsVault.BuyRoute memory route,
        uint256 amount,
        uint256 min_received,
        uint256 deadline
    ) private {
        try vault.executeBuy(envelope, route, amount, min_received, deadline) returns (uint256) {
            revert("expected executeBuy revert");
        } catch {}
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}

contract MockVenue {
    MockToken private immutable asset;
    uint256 public price_usd_e8;

    constructor(MockToken asset_) {
        asset = asset_;
    }

    function setPriceUsdE8(uint256 price_usd_e8_) external {
        price_usd_e8 = price_usd_e8_;
    }

    function executeBuy(
        address,
        address,
        uint256 reserve_amount,
        uint256 min_received,
        address recipient,
        bytes calldata
    ) external returns (uint256 reported_received) {
        reported_received = reserve_amount * 100 / price_usd_e8;
        if (reported_received < min_received) {
            revert("venue min received");
        }
        asset.mint(recipient, reported_received);
    }
}

contract MockToken {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    uint256 public totalSupply;

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        _transfer(msg.sender, to, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 current_allowance = allowance[from][msg.sender];
        if (current_allowance < amount) {
            return false;
        }
        allowance[from][msg.sender] = current_allowance - amount;
        _transfer(from, to, amount);
        return true;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }

    function burn(uint256 amount) external {
        uint256 balance = balanceOf[msg.sender];
        if (balance < amount) {
            revert("burn balance");
        }
        balanceOf[msg.sender] = balance - amount;
        totalSupply -= amount;
    }

    function _transfer(address from, address to, uint256 amount) private {
        uint256 balance = balanceOf[from];
        if (balance < amount) {
            revert("transfer balance");
        }
        balanceOf[from] = balance - amount;
        balanceOf[to] += amount;
    }
}
