// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "../src/MarketOpsEnvelope.sol";
import {PFTLBridgeAdapter} from "../src/PFTLBridgeAdapter.sol";
import {PolicyRegistry} from "../src/PolicyRegistry.sol";

interface Vm {
    function warp(uint256 timestamp) external;
}

contract PFTLBridgeAdapterTest {
    Vm private constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    PolicyRegistry private registry;
    PFTLBridgeAdapter private adapter;

    uint64 private constant CHAIN_ID = 65_100;
    uint64 private constant CHALLENGE_DELAY = 100;
    uint64 private constant EXECUTION_WINDOW = 1_000;
    uint64 private constant MAX_STALENESS = 75;

    address private constant VAULT = address(0x1212);
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
    bytes private constant OTHER_ENVELOPE_HASH =
        hex"3102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30";

    function setUp() public {
        registry = new PolicyRegistry(address(this));
        registry.registerPolicy(
            PROGRAM_ID, POLICY_HASH, PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 1, 0
        );
        adapter = new PFTLBridgeAdapter(
            registry, address(this), CHAIN_ID, VAULT, MINT_CONTROLLER, CHALLENGE_DELAY, EXECUTION_WINDOW, MAX_STALENESS
        );
    }

    function testValidEnvelopeFinalizesAccepted() public {
        vm.warp(1_000);
        MarketOpsEnvelope memory envelope = _envelope(1, 1_100, 1_600, 950);

        bytes32 pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);
        _assertTrue(pending_id != bytes32(0), "pending id");
        _assertEq(
            uint256(adapter.getEnvelopeStatus(pending_id)),
            uint256(PFTLBridgeAdapter.EnvelopeStatus.Pending),
            "pending status"
        );
        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 0, "pending cap zero");

        vm.warp(1_100);
        adapter.finalizeEnvelope(pending_id);

        _assertEq(
            uint256(adapter.getEnvelopeStatus(pending_id)),
            uint256(PFTLBridgeAdapter.EnvelopeStatus.Accepted),
            "accepted status"
        );
        _assertTrue(adapter.isEnvelopeExecutable(pending_id), "executable");
        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 25_875e8, "reserve cap");
        _assertEq(adapter.mintCapAtoms(pending_id), 8_300, "mint cap");
        _assertEq(
            adapter.accepted_envelope_by_asset_epoch(adapter.assetEpochKey(ASSET_ID, 1)), pending_id, "accepted index"
        );
    }

    function testStaleDataWindowRejected() public {
        vm.warp(1_000);
        MarketOpsEnvelope memory envelope = _envelope(1, 1_100, 1_600, 900);

        _expectSubmitRevert(envelope, ENVELOPE_HASH);
        _assertTrue(!adapter.paused(), "stale rejection does not pause");
    }

    function testChallengedEnvelopeFreezesAfterDelayAndCapsZero() public {
        vm.warp(1_000);
        MarketOpsEnvelope memory envelope = _envelope(1, 1_100, 1_600, 950);
        bytes32 pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);

        adapter.challengeEnvelope(pending_id, PFTLBridgeAdapter.ChallengeFault.HashMismatch);
        _assertEq(
            uint256(adapter.getEnvelopeStatus(pending_id)),
            uint256(PFTLBridgeAdapter.EnvelopeStatus.Challenged),
            "challenged status"
        );
        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 0, "challenged reserve cap");
        _assertEq(adapter.mintCapAtoms(pending_id), 0, "challenged mint cap");

        vm.warp(1_100);
        adapter.finalizeEnvelope(pending_id);

        _assertEq(
            uint256(adapter.getEnvelopeStatus(pending_id)),
            uint256(PFTLBridgeAdapter.EnvelopeStatus.Frozen),
            "frozen status"
        );
        _assertEq(
            uint256(adapter.getChallengeFault(pending_id)),
            uint256(PFTLBridgeAdapter.ChallengeFault.HashMismatch),
            "challenge fault"
        );
        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 0, "frozen cap");
    }

    function testEquivocationPausesAdapter() public {
        vm.warp(1_000);
        MarketOpsEnvelope memory envelope = _envelope(1, 1_100, 1_600, 950);
        bytes32 first_pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);
        _assertTrue(first_pending_id != bytes32(0), "first pending id");

        envelope.nonce = bytes32(uint256(0x56));
        bytes32 second_pending_id = adapter.submitEnvelope(envelope, OTHER_ENVELOPE_HASH);

        _assertEq(second_pending_id, bytes32(0), "equivocation returns zero id");
        _assertTrue(adapter.paused(), "adapter paused");
        _assertEq(adapter.reserveDeployCapUsdE8(first_pending_id), 0, "paused cap zero");
        _expectSubmitRevert(_envelope(2, 1_100, 1_600, 950), ENVELOPE_HASH);
    }

    function testTimingViolationsRejected() public {
        vm.warp(1_000);
        _expectSubmitRevert(_envelope(1, 1_099, 1_600, 950), ENVELOPE_HASH);
        _expectSubmitRevert(_envelope(1, 1_100, 2_101, 950), ENVELOPE_HASH);
        _expectSubmitRevert(_envelope(1, 900, 999, 950), ENVELOPE_HASH);
    }

    function testWrongPolicyAndBindingsRejected() public {
        vm.warp(1_000);
        MarketOpsEnvelope memory wrong_policy = _envelope(1, 1_100, 1_600, 950);
        wrong_policy.policy_hash = bytes32(uint256(0x9999));
        _expectSubmitRevert(wrong_policy, ENVELOPE_HASH);

        MarketOpsEnvelope memory wrong_binding = _envelope(1, 1_100, 1_600, 950);
        wrong_binding.vault_address = address(0x9999);
        _expectSubmitRevert(wrong_binding, ENVELOPE_HASH);
    }

    function _envelope(uint64 epoch, uint64 valid_after, uint64 expires_at, uint64 data_window_end)
        private
        view
        returns (MarketOpsEnvelope memory envelope)
    {
        envelope.encoding_version = 1;
        envelope.chain_id = CHAIN_ID;
        envelope.adapter_address = address(adapter);
        envelope.vault_address = VAULT;
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
        envelope.max_reserve_deploy_usd_e8 = 25_875e8;
        envelope.max_mint_atoms = 8_300;
        envelope.discount_trigger_bps = 300;
        envelope.premium_trigger_bps = 1_000;
        envelope.data_window_start = data_window_end - 100;
        envelope.data_window_end = data_window_end;
        envelope.valid_after = valid_after;
        envelope.expires_at = expires_at;
        envelope.cooldown_seconds = 600;
        envelope.nonce = bytes32(uint256(0x55));
    }

    function _expectSubmitRevert(MarketOpsEnvelope memory envelope, bytes memory envelope_hash) private {
        try adapter.submitEnvelope(envelope, envelope_hash) returns (bytes32) {
            revert("expected submitEnvelope revert");
        } catch {}
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) {
            revert(message);
        }
    }

    function _assertEq(bytes32 actual, bytes32 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}
