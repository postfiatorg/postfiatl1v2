// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IERC20BridgeToken, IPFTLWithdrawalVerifier, ERC20BridgeVault} from "../src/ERC20BridgeVault.sol";
import {PFTLWithdrawalVerifier} from "../src/PFTLWithdrawalVerifier.sol";

interface DeployVm {
    function envAddress(string calldata name) external returns (address);
    function envAddress(string calldata name, string calldata delimiter) external returns (address[] memory);
    function envBytes(string calldata name) external returns (bytes memory);
    function envUint(string calldata name) external returns (uint256);
    function startBroadcast(uint256 private_key) external;
    function stopBroadcast() external;
}

/// @notice Foundry deployment harness for one ERC20-backed PFTL vault bridge asset.
/// @dev No forge-std dependency is required; the script declares only the
///      cheatcodes it uses. All asset-specific values come from the environment.
contract DeployERC20Bridge {
    DeployVm private constant vm = DeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant PFTL_FIELD_HASH_BYTES = 48;

    error EmptySignerSet();
    error InvalidAssetIdLength(uint256 length);
    error Uint64Overflow(string field, uint256 value);

    event ERC20BridgeDeployed(
        address indexed withdrawal_verifier,
        address indexed vault,
        address indexed token,
        address owner,
        uint64 pftl_chain_id,
        bytes vault_bridge_asset_id
    );

    function run() external returns (PFTLWithdrawalVerifier withdrawal_verifier, ERC20BridgeVault vault) {
        uint256 private_key = vm.envUint("PRIVATE_KEY");
        address token = vm.envAddress("ERC20_BRIDGE_TOKEN");
        address owner = vm.envAddress("PFTL_BRIDGE_OWNER");
        address[] memory signers = vm.envAddress("PFTL_WITHDRAWAL_SIGNERS", ",");
        if (signers.length == 0) {
            revert EmptySignerSet();
        }

        uint256 threshold = vm.envUint("PFTL_WITHDRAWAL_THRESHOLD");
        uint64 pftl_chain_id = _uint64("PFTL_CHAIN_ID", vm.envUint("PFTL_CHAIN_ID"));
        bytes memory vault_bridge_asset_id = vm.envBytes("VAULT_BRIDGE_ASSET_ID");
        if (vault_bridge_asset_id.length != PFTL_FIELD_HASH_BYTES) {
            revert InvalidAssetIdLength(vault_bridge_asset_id.length);
        }

        uint64 verifier_challenge_delay =
            _uint64("PFTL_WITHDRAWAL_CHALLENGE_DELAY_SECONDS", vm.envUint("PFTL_WITHDRAWAL_CHALLENGE_DELAY_SECONDS"));
        uint64 verifier_execution_window =
            _uint64("PFTL_WITHDRAWAL_EXECUTION_WINDOW_SECONDS", vm.envUint("PFTL_WITHDRAWAL_EXECUTION_WINDOW_SECONDS"));
        uint64 vault_challenge_delay = _uint64(
            "ERC20_BRIDGE_VAULT_CHALLENGE_DELAY_SECONDS", vm.envUint("ERC20_BRIDGE_VAULT_CHALLENGE_DELAY_SECONDS")
        );
        uint64 vault_execution_window = _uint64(
            "ERC20_BRIDGE_VAULT_EXECUTION_WINDOW_SECONDS", vm.envUint("ERC20_BRIDGE_VAULT_EXECUTION_WINDOW_SECONDS")
        );

        vm.startBroadcast(private_key);
        withdrawal_verifier =
            new PFTLWithdrawalVerifier(owner, signers, threshold, verifier_challenge_delay, verifier_execution_window);
        vault = new ERC20BridgeVault(
            IERC20BridgeToken(token),
            IPFTLWithdrawalVerifier(address(withdrawal_verifier)),
            owner,
            pftl_chain_id,
            vault_bridge_asset_id,
            vault_challenge_delay,
            vault_execution_window
        );
        vm.stopBroadcast();

        emit ERC20BridgeDeployed(
            address(withdrawal_verifier), address(vault), token, owner, pftl_chain_id, vault_bridge_asset_id
        );
    }

    function _uint64(string memory field, uint256 value) private pure returns (uint64) {
        if (value > type(uint64).max) {
            revert Uint64Overflow(field, value);
        }
        // forge-lint: disable-next-line(unsafe-typecast)
        return uint64(value);
    }
}
