// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {WrappedVenueNAVCoin} from "../src/PFTLUniswapHandoffController.sol";
import {
    IPFTLReceiptSP1Verifier,
    PFTLReceiptFinalityVerifierV1
} from "../src/PFTLReceiptFinalityVerifierV1.sol";
import {
    IA666WrappedToken,
    IPFTLReceiptFinalityVerifierV1,
    PFTLUniswapPrimaryMarketV2
} from "../src/PFTLUniswapPrimaryMarketV2.sol";

interface DeployA666Vm {
    function envAddress(string calldata name) external returns (address);
    function envBytes(string calldata name) external returns (bytes memory);
    function envBytes32(string calldata name) external returns (bytes32);
    function envUint(string calldata name) external returns (uint256);
    function addr(uint256 privateKey) external returns (address);
    function getNonce(address account) external returns (uint64);
    function computeCreateAddress(uint256 nonce, address deployer) external returns (address);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

/// @notice Deterministic a666 v2 deployment script.
/// @dev Running without Foundry's `--broadcast` produces the required
///      no-broadcast package simulation. The controller address is predicted
///      from the deployer nonce so the verifier can pin it before creation.
contract DeployA666PrimaryMarket {
    DeployA666Vm private constant vm =
        DeployA666Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    error InvalidPftlBytes(string field, uint256 length);
    error Uint64Overflow(string field, uint256 value);
    error PredictedAddressMismatch(address predicted, address actual);
    error RuntimeCodeHashMismatch(bytes32 expected, bytes32 actual);

    struct Params {
        address governance;
        bytes32 routeIdCommitment;
        bytes routeConfigDigest;
        bytes settlementAssetId;
        bytes nativeNavAssetId;
        bytes pricingReservePacketHash;
        bytes32 policyHashCommitment;
        bytes32 expectedControllerRuntimeCodeHash;
        uint64 routeEpoch;
        uint64 pricingNavEpoch;
        uint64 initialHeight;
        bytes32 poolId;
    }

    event A666PrimaryMarketDeployed(
        address indexed wrappedToken,
        address indexed receiptVerifier,
        address indexed controller,
        address governance,
        bytes32 routeIdCommitment,
        bytes32 policyHashCommitment,
        bytes32 poolId,
        uint64 routeEpoch,
        bytes32 controllerRuntimeCodeHash
    );

    function run()
        external
        returns (
            WrappedVenueNAVCoin wrappedToken,
            PFTLReceiptFinalityVerifierV1 receiptVerifier,
            PFTLUniswapPrimaryMarketV2 controller
        )
    {
        uint256 privateKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(privateKey);
        uint64 nonce = vm.getNonce(deployer);
        address predictedController = vm.computeCreateAddress(uint256(nonce) + 2, deployer);
        Params memory params = _params();

        vm.startBroadcast(privateKey);
        wrappedToken = new WrappedVenueNAVCoin("Post Fiat a666", "wA666", 6, deployer);
        receiptVerifier = _deployVerifier(wrappedToken, predictedController, params);
        controller = _deployController(wrappedToken, receiptVerifier, params);
        if (address(controller) != predictedController) {
            revert PredictedAddressMismatch(predictedController, address(controller));
        }
        if (
            params.expectedControllerRuntimeCodeHash != bytes32(0)
                && address(controller).codehash != params.expectedControllerRuntimeCodeHash
        ) {
            revert RuntimeCodeHashMismatch(
                params.expectedControllerRuntimeCodeHash, address(controller).codehash
            );
        }
        wrappedToken.setController(address(controller));
        wrappedToken.lockController();
        wrappedToken.transferOwnership(params.governance);
        vm.stopBroadcast();

        emit A666PrimaryMarketDeployed(
            address(wrappedToken),
            address(receiptVerifier),
            address(controller),
            params.governance,
            params.routeIdCommitment,
            params.policyHashCommitment,
            params.poolId,
            params.routeEpoch,
            address(controller).codehash
        );
    }

    function _params() private returns (Params memory params) {
        params = Params({
            governance: vm.envAddress("A666_GOVERNANCE"),
            routeIdCommitment: vm.envBytes32("A666_ROUTE_ID_COMMITMENT"),
            routeConfigDigest: _pftlBytes("A666_ROUTE_CONFIG_DIGEST"),
            settlementAssetId: _pftlBytes("A666_SETTLEMENT_ASSET_ID"),
            nativeNavAssetId: _pftlBytes("A666_NATIVE_NAV_ASSET_ID"),
            pricingReservePacketHash: _pftlBytes("A666_PRICING_RESERVE_PACKET_HASH"),
            policyHashCommitment: vm.envBytes32("A666_POLICY_HASH_COMMITMENT"),
            expectedControllerRuntimeCodeHash: vm.envBytes32("A666_CONTROLLER_RUNTIME_CODE_HASH"),
            routeEpoch: _uint64("A666_ROUTE_EPOCH", vm.envUint("A666_ROUTE_EPOCH")),
            pricingNavEpoch: _uint64(
                "A666_PRICING_NAV_EPOCH", vm.envUint("A666_PRICING_NAV_EPOCH")
            ),
            initialHeight: _uint64(
                "A666_INITIAL_FINALIZED_HEIGHT", vm.envUint("A666_INITIAL_FINALIZED_HEIGHT")
            ),
            poolId: vm.envBytes32("A666_UNISWAP_POOL_ID")
        });
    }

    function _deployVerifier(
        WrappedVenueNAVCoin wrappedToken,
        address predictedController,
        Params memory params
    ) private returns (PFTLReceiptFinalityVerifierV1) {
        uint256 protocolVersion = vm.envUint("PFTL_PROTOCOL_VERSION");
        if (protocolVersion > type(uint32).max) {
            revert Uint64Overflow("PFTL_PROTOCOL_VERSION", protocolVersion);
        }
        return new PFTLReceiptFinalityVerifierV1(
            PFTLReceiptFinalityVerifierV1.Config({
                sp1Verifier: IPFTLReceiptSP1Verifier(vm.envAddress("SP1_VERIFIER")),
                programVKey: vm.envBytes32("A666_RECEIPT_PROGRAM_VKEY"),
                pftlChainIdHash: vm.envBytes32("PFTL_CHAIN_ID_HASH"),
                pftlGenesisHashCommitment: vm.envBytes32("PFTL_GENESIS_HASH_COMMITMENT"),
                pftlProtocolVersion: uint32(protocolVersion),
                routeIdCommitment: params.routeIdCommitment,
                nativeNavAssetIdCommitment: keccak256(params.nativeNavAssetId),
                settlementAssetIdCommitment: keccak256(params.settlementAssetId),
                destinationChainId: block.chainid,
                controller: predictedController,
                wrappedToken: address(wrappedToken),
                wrappedTokenRuntimeCodeHash: address(wrappedToken).codehash,
                maxProofBytes: vm.envUint("A666_MAX_PROOF_BYTES"),
                maxPublicValuesBytes: 1120,
                initialCheckpointCommitment: vm.envBytes32("A666_INITIAL_CHECKPOINT_COMMITMENT"),
                initialFinalizedHeight: params.initialHeight
            })
        );
    }

    function _deployController(
        WrappedVenueNAVCoin wrappedToken,
        PFTLReceiptFinalityVerifierV1 receiptVerifier,
        Params memory params
    ) private returns (PFTLUniswapPrimaryMarketV2) {
        return new PFTLUniswapPrimaryMarketV2(
            IA666WrappedToken(address(wrappedToken)),
            IPFTLReceiptFinalityVerifierV1(address(receiptVerifier)),
            PFTLUniswapPrimaryMarketV2.Config({
                destinationChainId: block.chainid,
                settlementAssetId: params.settlementAssetId,
                nativeNavAssetId: params.nativeNavAssetId,
                uniswapPoolId: params.poolId,
                routeSupplyCapAtoms: 2_000_000e6,
                packetNotionalCapAtoms: 250_000e6,
                governance: params.governance
            })
        );
    }

    function _pftlBytes(string memory name) private returns (bytes memory value) {
        value = vm.envBytes(name);
        if (value.length != 48) revert InvalidPftlBytes(name, value.length);
    }

    function _uint64(string memory field, uint256 value) private pure returns (uint64) {
        if (value > type(uint64).max) revert Uint64Overflow(field, value);
        return uint64(value);
    }
}
