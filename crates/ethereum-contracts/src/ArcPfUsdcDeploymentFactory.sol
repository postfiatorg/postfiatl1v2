// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    ERC20BridgeVaultV2,
    IArbSysPfUsdcV1,
    IERC20BridgeTokenV2,
    IPFTLFinalityVerifierV1
} from "./ERC20BridgeVaultV2.sol";
import {IArbitrumBridgeV1, PfUsdcIngressAnchorV1} from "./PfUsdcIngressAnchorV1.sol";

/// @notice One-shot factory that resolves the Arc anchor/vault address cycle.
/// @dev A fresh contract's first and second CREATE operations use nonces 1 and 2.
///      The anchor can therefore pin the exact vault address before the vault exists.
contract ArcPfUsdcDeploymentFactory {
    error NotDeployer();
    error AlreadyDeployed();
    error AddressPredictionFailed(bytes32 contractKind, address expected, address actual);

    event ArcPfUsdcContractsDeployed(
        address indexed anchor,
        address indexed vault,
        address indexed token,
        address finalityVerifier,
        address owner,
        bytes32 routeBinding
    );

    address public immutable deployer;
    bool public deployed;

    constructor() {
        deployer = msg.sender;
    }

    function predictedAnchor() public view returns (address) {
        return _createAddress(1);
    }

    function predictedVault() public view returns (address) {
        return _createAddress(2);
    }

    function deploy(
        IERC20BridgeTokenV2 token,
        IPFTLFinalityVerifierV1 finalityVerifier,
        bytes32 routeBinding,
        address owner
    ) external returns (PfUsdcIngressAnchorV1 anchor, ERC20BridgeVaultV2 vault) {
        if (msg.sender != deployer) revert NotDeployer();
        if (deployed) revert AlreadyDeployed();
        deployed = true;

        address expectedAnchor = predictedAnchor();
        address expectedVault = predictedVault();
        anchor = new PfUsdcIngressAnchorV1(
            IArbitrumBridgeV1(address(0)), expectedVault, address(token), block.chainid, routeBinding
        );
        if (address(anchor) != expectedAnchor) {
            revert AddressPredictionFailed("anchor", expectedAnchor, address(anchor));
        }

        vault = new ERC20BridgeVaultV2(
            token, finalityVerifier, address(token).codehash, IArbSysPfUsdcV1(address(0)), address(anchor), owner
        );
        if (address(vault) != expectedVault) {
            revert AddressPredictionFailed("vault", expectedVault, address(vault));
        }

        emit ArcPfUsdcContractsDeployed(
            address(anchor), address(vault), address(token), address(finalityVerifier), owner, routeBinding
        );
    }

    function _createAddress(uint8 nonce) private view returns (address) {
        return address(uint160(uint256(keccak256(abi.encodePacked(hex"d694", address(this), nonce)))));
    }
}
