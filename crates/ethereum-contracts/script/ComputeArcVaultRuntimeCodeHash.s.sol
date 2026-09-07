// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    ERC20BridgeVaultV2,
    IArbSysPfUsdcV1,
    IERC20BridgeTokenV2,
    IPFTLFinalityVerifierV1
} from "../src/ERC20BridgeVaultV2.sol";
import {ArcPfUsdcDeploymentFactory} from "../src/ArcPfUsdcDeploymentFactory.sol";

interface ArcVaultHashVm {
    function envAddress(string calldata name) external returns (address);
}

/// @notice Simulation-only immutable resolver for the Arc vault runtime hash.
/// @dev Run without --broadcast on an Arc testnet fork. The temporary vault
///      exists only inside the simulation and must never produce a transaction.
contract ComputeArcVaultRuntimeCodeHash {
    ArcVaultHashVm private constant vm = ArcVaultHashVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant ARC_TESTNET_CHAIN_ID = 5_042_002;
    address private constant ARC_TESTNET_USDC = 0x3600000000000000000000000000000000000000;

    error WrongChain(uint256 actual);
    error WrongValue(bytes32 field);

    event ArcVaultRuntimeCodeHashComputed(
        address indexed predictedVault,
        address indexed predictedAnchor,
        bytes32 vaultRuntimeCodeHash,
        bytes32 tokenRuntimeCodeHash
    );

    function run() external returns (bytes32 vaultRuntimeCodeHash) {
        if (block.chainid != ARC_TESTNET_CHAIN_ID) revert WrongChain(block.chainid);
        address owner = vm.envAddress("PFUSDC_OWNER");
        if (owner == address(0)) revert WrongValue("owner");
        ArcPfUsdcDeploymentFactory factory = ArcPfUsdcDeploymentFactory(vm.envAddress("ARC_PFUSDC_FACTORY"));
        if (factory.deployed()) revert WrongValue("factory_deployed");

        bytes32 tokenRuntimeCodeHash = ARC_TESTNET_USDC.codehash;
        if (tokenRuntimeCodeHash == bytes32(0)) revert WrongValue("token_code_hash");
        ERC20BridgeVaultV2 probe = new ERC20BridgeVaultV2(
            IERC20BridgeTokenV2(ARC_TESTNET_USDC),
            IPFTLFinalityVerifierV1(address(1)),
            tokenRuntimeCodeHash,
            IArbSysPfUsdcV1(address(0)),
            factory.predictedAnchor(),
            owner
        );
        vaultRuntimeCodeHash = address(probe).codehash;
        emit ArcVaultRuntimeCodeHashComputed(
            factory.predictedVault(), factory.predictedAnchor(), vaultRuntimeCodeHash, tokenRuntimeCodeHash
        );
    }
}
