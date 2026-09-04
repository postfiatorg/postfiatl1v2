// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.20;

import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";
import {SP1Verifier} from "@sp1-contracts/v6.1.0/SP1VerifierGroth16.sol";

interface ArcSp1DeployVm {
    function addr(uint256 privateKey) external returns (address);
    function envAddress(string calldata name) external returns (address);
    function envUint(string calldata name) external returns (uint256);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

/// @notice Deploys the exact SP1 v6.1.0 Groth16 verifier route required by
///         sp1-sdk 6.3.1 and registers it in a new Arc-local gateway.
/// @dev The gateway owner must be the deploying EOA because route registration
///      is performed atomically by this script. Ownership can be transferred
///      after the route readback has been recorded.
contract DeployArcSp1Verifier {
    ArcSp1DeployVm private constant vm =
        ArcSp1DeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    bytes32 public constant EXPECTED_VERIFIER_HASH =
        0x4388a21c687fdd5f218d7e3d13190cac4c5355818d3605fd5fb811df468ee696;
    // forge-lint: disable-next-line(unsafe-typecast)
    bytes4 public constant EXPECTED_SELECTOR = bytes4(EXPECTED_VERIFIER_HASH);

    error GatewayOwnerMustBeDeployer(address owner, address deployer);
    error WrongVerifierHash(bytes32 actual);
    error WrongRoute(address verifier, bool frozen);

    event ArcSp1VerifierDeployed(
        address indexed gateway,
        address indexed verifier,
        address indexed owner,
        bytes32 verifierHash,
        bytes4 selector
    );

    function run() external returns (SP1VerifierGateway gateway, SP1Verifier verifier) {
        uint256 privateKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(privateKey);
        address owner = vm.envAddress("SP1_GATEWAY_OWNER");
        if (owner != deployer) revert GatewayOwnerMustBeDeployer(owner, deployer);

        vm.startBroadcast(privateKey);
        gateway = new SP1VerifierGateway(owner);
        verifier = new SP1Verifier();
        gateway.addRoute(address(verifier));
        vm.stopBroadcast();

        bytes32 verifierHash = verifier.VERIFIER_HASH();
        if (verifierHash != EXPECTED_VERIFIER_HASH) revert WrongVerifierHash(verifierHash);
        (address routedVerifier, bool frozen) = gateway.routes(EXPECTED_SELECTOR);
        if (routedVerifier != address(verifier) || frozen) {
            revert WrongRoute(routedVerifier, frozen);
        }

        emit ArcSp1VerifierDeployed(
            address(gateway), address(verifier), owner, verifierHash, EXPECTED_SELECTOR
        );
    }
}
