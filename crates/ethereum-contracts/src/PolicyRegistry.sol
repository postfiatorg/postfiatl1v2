// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "./MarketOpsEnvelope.sol";

/// @notice Registry of PFTL market-policy identities accepted by Ethereum contracts.
/// @dev Policy records are immutable after registration except for explicit deactivation.
contract PolicyRegistry {
    struct Policy {
        bytes32 program_id;
        bytes32 policy_hash;
        bytes32 parameter_hash;
        bytes32 venue_id;
        bytes32 pool_config_hash;
        bytes32 hook_code_hash;
        uint64 activation_epoch;
        uint64 deactivation_epoch;
        bool registered;
    }

    error NotOwner();
    error ZeroOwner();
    error ZeroField(bytes32 field);
    error InvalidDeactivationEpoch(uint64 activation_epoch, uint64 deactivation_epoch);
    error PolicyAlreadyRegistered(bytes32 policy_id);
    error PolicyNotRegistered(bytes32 policy_id);
    error PolicyAlreadyDeactivated(bytes32 policy_id, uint64 deactivation_epoch);
    error ParameterHashChanged(
        bytes32 parameter_identity, bytes32 existing_parameter_hash, bytes32 attempted_parameter_hash
    );

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event PolicyRegistered(
        bytes32 indexed policy_id,
        bytes32 indexed program_id,
        bytes32 indexed policy_hash,
        bytes32 parameter_hash,
        bytes32 venue_id,
        bytes32 pool_config_hash,
        bytes32 hook_code_hash,
        uint64 activation_epoch,
        uint64 deactivation_epoch
    );
    event PolicyDeactivated(bytes32 indexed policy_id, uint64 deactivation_epoch);

    address public owner;

    mapping(bytes32 => Policy) private policies;
    mapping(bytes32 => bytes32) public parameter_hash_by_identity;
    mapping(bytes32 => bool) public is_eligible_venue;
    bytes32[] private eligible_venue_ids;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    constructor(address initial_owner) {
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        owner = initial_owner;
        emit OwnershipTransferred(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function registerPolicy(
        bytes32 program_id,
        bytes32 policy_hash,
        bytes32 parameter_hash,
        bytes32 venue_id,
        bytes32 pool_config_hash,
        bytes32 hook_code_hash,
        uint64 activation_epoch,
        uint64 deactivation_epoch
    ) external onlyOwner returns (bytes32 policy_id) {
        _validatePolicyFields(
            program_id,
            policy_hash,
            parameter_hash,
            venue_id,
            pool_config_hash,
            hook_code_hash,
            activation_epoch,
            deactivation_epoch
        );

        policy_id =
            policyIdForFields(program_id, policy_hash, parameter_hash, venue_id, pool_config_hash, hook_code_hash);
        if (policies[policy_id].registered) {
            revert PolicyAlreadyRegistered(policy_id);
        }

        bytes32 parameter_identity =
            parameterIdentityForFields(program_id, policy_hash, venue_id, pool_config_hash, hook_code_hash);
        bytes32 existing_parameter_hash = parameter_hash_by_identity[parameter_identity];
        if (existing_parameter_hash != bytes32(0) && existing_parameter_hash != parameter_hash) {
            revert ParameterHashChanged(parameter_identity, existing_parameter_hash, parameter_hash);
        }
        parameter_hash_by_identity[parameter_identity] = parameter_hash;

        policies[policy_id] = Policy({
            program_id: program_id,
            policy_hash: policy_hash,
            parameter_hash: parameter_hash,
            venue_id: venue_id,
            pool_config_hash: pool_config_hash,
            hook_code_hash: hook_code_hash,
            activation_epoch: activation_epoch,
            deactivation_epoch: deactivation_epoch,
            registered: true
        });

        if (!is_eligible_venue[venue_id]) {
            is_eligible_venue[venue_id] = true;
            eligible_venue_ids.push(venue_id);
        }

        emit PolicyRegistered(
            policy_id,
            program_id,
            policy_hash,
            parameter_hash,
            venue_id,
            pool_config_hash,
            hook_code_hash,
            activation_epoch,
            deactivation_epoch
        );
    }

    function deactivatePolicy(bytes32 policy_id, uint64 deactivation_epoch) external onlyOwner {
        Policy storage policy = policies[policy_id];
        if (!policy.registered) {
            revert PolicyNotRegistered(policy_id);
        }
        if (policy.deactivation_epoch != 0) {
            revert PolicyAlreadyDeactivated(policy_id, policy.deactivation_epoch);
        }
        if (deactivation_epoch <= policy.activation_epoch) {
            revert InvalidDeactivationEpoch(policy.activation_epoch, deactivation_epoch);
        }

        policy.deactivation_epoch = deactivation_epoch;
        emit PolicyDeactivated(policy_id, deactivation_epoch);
    }

    function getPolicy(bytes32 policy_id) external view returns (Policy memory) {
        return policies[policy_id];
    }

    function isPolicyRegistered(bytes32 policy_id) external view returns (bool) {
        return policies[policy_id].registered;
    }

    function isPolicyActive(bytes32 policy_id, uint64 epoch) public view returns (bool) {
        Policy storage policy = policies[policy_id];
        return policy.registered && epoch >= policy.activation_epoch
            && (policy.deactivation_epoch == 0 || epoch < policy.deactivation_epoch);
    }

    function isEnvelopeAccepted(MarketOpsEnvelope calldata envelope) external view returns (bool) {
        bytes32 policy_id = policyIdForEnvelope(envelope);
        Policy storage policy = policies[policy_id];
        return policy.registered && policy.program_id == envelope.program_id
            && policy.policy_hash == envelope.policy_hash && policy.parameter_hash == envelope.parameter_hash
            && policy.venue_id == envelope.venue_id && policy.pool_config_hash == envelope.pool_config_hash
            && policy.hook_code_hash == envelope.hook_code_hash && envelope.epoch >= policy.activation_epoch
            && (policy.deactivation_epoch == 0 || envelope.epoch < policy.deactivation_epoch);
    }

    function eligibleVenueCount() external view returns (uint256) {
        return eligible_venue_ids.length;
    }

    function eligibleVenueIdAt(uint256 index) external view returns (bytes32) {
        return eligible_venue_ids[index];
    }

    function policyIdForEnvelope(MarketOpsEnvelope calldata envelope) public pure returns (bytes32) {
        return policyIdForFields(
            envelope.program_id,
            envelope.policy_hash,
            envelope.parameter_hash,
            envelope.venue_id,
            envelope.pool_config_hash,
            envelope.hook_code_hash
        );
    }

    function policyIdForFields(
        bytes32 program_id,
        bytes32 policy_hash,
        bytes32 parameter_hash,
        bytes32 venue_id,
        bytes32 pool_config_hash,
        bytes32 hook_code_hash
    ) public pure returns (bytes32) {
        return keccak256(
            abi.encode(program_id, policy_hash, parameter_hash, venue_id, pool_config_hash, hook_code_hash)
        );
    }

    function parameterIdentityForFields(
        bytes32 program_id,
        bytes32 policy_hash,
        bytes32 venue_id,
        bytes32 pool_config_hash,
        bytes32 hook_code_hash
    ) public pure returns (bytes32) {
        return keccak256(abi.encode(program_id, policy_hash, venue_id, pool_config_hash, hook_code_hash));
    }

    function _validatePolicyFields(
        bytes32 program_id,
        bytes32 policy_hash,
        bytes32 parameter_hash,
        bytes32 venue_id,
        bytes32 pool_config_hash,
        bytes32 hook_code_hash,
        uint64 activation_epoch,
        uint64 deactivation_epoch
    ) private pure {
        if (program_id == bytes32(0)) {
            revert ZeroField("program_id");
        }
        if (policy_hash == bytes32(0)) {
            revert ZeroField("policy_hash");
        }
        if (parameter_hash == bytes32(0)) {
            revert ZeroField("parameter_hash");
        }
        if (venue_id == bytes32(0)) {
            revert ZeroField("venue_id");
        }
        if (pool_config_hash == bytes32(0)) {
            revert ZeroField("pool_config_hash");
        }
        if (hook_code_hash == bytes32(0)) {
            revert ZeroField("hook_code_hash");
        }
        if (deactivation_epoch != 0 && deactivation_epoch <= activation_epoch) {
            revert InvalidDeactivationEpoch(activation_epoch, deactivation_epoch);
        }
    }
}
