// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ArcPfUsdcDeploymentFactory} from "../src/ArcPfUsdcDeploymentFactory.sol";

interface ArcFactoryDeployVm {
    function addr(uint256 privateKey) external returns (address);
    function envUint(string calldata name) external returns (uint256);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

/// @notice Phase one of the Arc deployment. The factory must exist before the
///         route profile is finalized because its predicted vault address is a
///         committed route field.
contract DeployArcPfUsdcFactory {
    ArcFactoryDeployVm private constant vm =
        ArcFactoryDeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant ARC_TESTNET_CHAIN_ID = 5_042_002;

    error WrongChain(uint256 actual);
    error WrongDeployer(address actual, address expected);

    event ArcPfUsdcFactoryPrepared(
        address indexed factory, address indexed deployer, address predictedAnchor, address predictedVault
    );

    function run() external returns (ArcPfUsdcDeploymentFactory factory) {
        if (block.chainid != ARC_TESTNET_CHAIN_ID) revert WrongChain(block.chainid);
        uint256 privateKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(privateKey);

        vm.startBroadcast(privateKey);
        factory = new ArcPfUsdcDeploymentFactory();
        vm.stopBroadcast();

        if (factory.deployer() != deployer) {
            revert WrongDeployer(factory.deployer(), deployer);
        }
        emit ArcPfUsdcFactoryPrepared(address(factory), deployer, factory.predictedAnchor(), factory.predictedVault());
    }
}
