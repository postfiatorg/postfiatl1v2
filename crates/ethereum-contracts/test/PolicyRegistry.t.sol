// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "../src/MarketOpsEnvelope.sol";
import {PolicyRegistry} from "../src/PolicyRegistry.sol";

contract PolicyRegistryTest {
    PolicyRegistry private registry;

    bytes32 private constant PROGRAM_ID = bytes32(uint256(0x31));
    bytes32 private constant POLICY_HASH = bytes32(uint256(0x32));
    bytes32 private constant PARAMETER_HASH = bytes32(uint256(0x33));
    bytes32 private constant OTHER_PARAMETER_HASH = bytes32(uint256(0x34));
    bytes32 private constant VENUE_ID = bytes32(uint256(0x37));
    bytes32 private constant POOL_CONFIG_HASH = bytes32(uint256(0x38));
    bytes32 private constant HOOK_CODE_HASH = bytes32(uint256(0x39));

    function setUp() public {
        registry = new PolicyRegistry(address(this));
    }

    function testRegisterPolicyVerifyLookupAndEnvelopeAcceptance() public {
        bytes32 policy_id = _registerPolicy(10, 0);

        PolicyRegistry.Policy memory policy = registry.getPolicy(policy_id);
        _assertTrue(policy.registered, "policy registered");
        _assertEq(policy.program_id, PROGRAM_ID, "program id");
        _assertEq(policy.policy_hash, POLICY_HASH, "policy hash");
        _assertEq(policy.parameter_hash, PARAMETER_HASH, "parameter hash");
        _assertEq(policy.venue_id, VENUE_ID, "venue id");
        _assertEq(policy.pool_config_hash, POOL_CONFIG_HASH, "pool config hash");
        _assertEq(policy.hook_code_hash, HOOK_CODE_HASH, "hook code hash");
        _assertEq64(policy.activation_epoch, 10, "activation epoch");
        _assertEq64(policy.deactivation_epoch, 0, "deactivation epoch");

        _assertTrue(registry.is_eligible_venue(VENUE_ID), "eligible venue");
        _assertEq(registry.eligibleVenueCount(), 1, "eligible venue count");
        _assertEq(registry.eligibleVenueIdAt(0), VENUE_ID, "eligible venue at 0");
        _assertTrue(!registry.isPolicyActive(policy_id, 9), "inactive before activation");
        _assertTrue(registry.isPolicyActive(policy_id, 10), "active at activation");

        MarketOpsEnvelope memory envelope = _envelope(10);
        _assertEq(registry.policyIdForEnvelope(envelope), policy_id, "envelope policy id");
        _assertTrue(registry.isEnvelopeAccepted(envelope), "envelope accepted");

        envelope.parameter_hash = OTHER_PARAMETER_HASH;
        _assertTrue(!registry.isEnvelopeAccepted(envelope), "wrong parameter rejected");
    }

    function testDeactivationStopsFutureAcceptance() public {
        bytes32 policy_id = _registerPolicy(10, 0);

        registry.deactivatePolicy(policy_id, 30);
        PolicyRegistry.Policy memory policy = registry.getPolicy(policy_id);
        _assertEq64(policy.deactivation_epoch, 30, "deactivation epoch");

        _assertTrue(registry.isPolicyActive(policy_id, 29), "active before deactivation");
        _assertTrue(!registry.isPolicyActive(policy_id, 30), "inactive at deactivation");
        _assertTrue(!registry.isEnvelopeAccepted(_envelope(30)), "envelope not accepted");

        _expectRevertDeactivate(policy_id, 40);
    }

    function testRegisterPolicyWithInitialDeactivation() public {
        bytes32 policy_id = _registerPolicy(10, 20);
        _assertTrue(registry.isPolicyActive(policy_id, 19), "active before initial deactivation");
        _assertTrue(!registry.isPolicyActive(policy_id, 20), "inactive at initial deactivation");
    }

    function testPreventsSilentParameterChangesAndDuplicateRegistration() public {
        bytes32 policy_id = _registerPolicy(10, 0);

        _expectRevertRegister(
            PROGRAM_ID, POLICY_HASH, PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 10, 0
        );
        _assertTrue(registry.isPolicyRegistered(policy_id), "original policy still registered");

        _expectRevertRegister(
            PROGRAM_ID, POLICY_HASH, OTHER_PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 10, 0
        );
        _assertEq(
            registry.parameter_hash_by_identity(
                registry.parameterIdentityForFields(PROGRAM_ID, POLICY_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH)
            ),
            PARAMETER_HASH,
            "parameter hash remains pinned"
        );
    }

    function testRejectsInvalidRegistrationEpochsAndZeroFields() public {
        _expectRevertRegister(
            PROGRAM_ID, POLICY_HASH, PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 10, 10
        );
        _expectRevertRegister(
            bytes32(0), POLICY_HASH, PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 10, 0
        );
    }

    function _registerPolicy(uint64 activation_epoch, uint64 deactivation_epoch) private returns (bytes32) {
        return registry.registerPolicy(
            PROGRAM_ID,
            POLICY_HASH,
            PARAMETER_HASH,
            VENUE_ID,
            POOL_CONFIG_HASH,
            HOOK_CODE_HASH,
            activation_epoch,
            deactivation_epoch
        );
    }

    function _expectRevertRegister(
        bytes32 program_id,
        bytes32 policy_hash,
        bytes32 parameter_hash,
        bytes32 venue_id,
        bytes32 pool_config_hash,
        bytes32 hook_code_hash,
        uint64 activation_epoch,
        uint64 deactivation_epoch
    ) private {
        try registry.registerPolicy(
            program_id,
            policy_hash,
            parameter_hash,
            venue_id,
            pool_config_hash,
            hook_code_hash,
            activation_epoch,
            deactivation_epoch
        ) returns (
            bytes32
        ) {
            revert("expected registerPolicy revert");
        } catch {}
    }

    function _expectRevertDeactivate(bytes32 policy_id, uint64 deactivation_epoch) private {
        try registry.deactivatePolicy(policy_id, deactivation_epoch) {
            revert("expected deactivatePolicy revert");
        } catch {}
    }

    function _envelope(uint64 epoch) private pure returns (MarketOpsEnvelope memory envelope) {
        envelope.encoding_version = 1;
        envelope.chain_id = 1;
        envelope.adapter_address = address(0x1111);
        envelope.vault_address = address(0x1212);
        envelope.mint_controller_address = address(0x1313);
        envelope.asset_id = bytes32(uint256(0xaaaaaaaa));
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
        envelope.max_mint_atoms = 0;
        envelope.discount_trigger_bps = 300;
        envelope.premium_trigger_bps = 1_000;
        envelope.data_window_start = 100;
        envelope.data_window_end = 10_100;
        envelope.valid_after = 10_100;
        envelope.expires_at = 20_100;
        envelope.cooldown_seconds = 600;
        envelope.nonce = bytes32(uint256(0x55));
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

    function _assertEq64(uint64 actual, uint64 expected, string memory message) private pure {
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
