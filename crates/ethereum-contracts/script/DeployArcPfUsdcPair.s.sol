// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ERC20BridgeVaultV2, IERC20BridgeTokenV2, IPFTLFinalityVerifierV1} from "../src/ERC20BridgeVaultV2.sol";
import {ArcPfUsdcDeploymentFactory} from "../src/ArcPfUsdcDeploymentFactory.sol";
import {PfUsdcIngressAnchorV1} from "../src/PfUsdcIngressAnchorV1.sol";
import {ISP1Verifier, PFTLFinalityVerifierV1} from "../src/PFTLFinalityVerifierV1.sol";

interface ArcPairDeployVm {
    function addr(uint256 privateKey) external returns (address);
    function envAddress(string calldata name) external returns (address);
    function envBytes(string calldata name) external returns (bytes memory);
    function envBytes32(string calldata name) external returns (bytes32);
    function envString(string calldata name) external returns (string memory);
    function envUint(string calldata name) external returns (uint256);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

interface IArcSp1Gateway {
    function routes(bytes4 selector) external view returns (address verifier, bool frozen);
}

/// @notice Phase two of the Arc deployment. It validates all ceremony inputs,
///         computes ABI commitments, pins the actual vault runtime code hash,
///         and atomically deploys the finality verifier plus factory pair.
contract DeployArcPfUsdcPair {
    ArcPairDeployVm private constant vm = ArcPairDeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant ARC_TESTNET_CHAIN_ID = 5_042_002;
    address private constant ARC_TESTNET_USDC = 0x3600000000000000000000000000000000000000;
    bytes32 private constant EGRESS_PROGRAM_VKEY = 0x0026a156bfd82ce1d1bf3f966c77daba8d5c266b8cc29928474747c4a02ca89b;
    bytes32 private constant SP1_VERIFIER_RUNTIME_CODE_HASH =
        0xc26a6452cb4fb09bc555e9ba44384da0267da540ec8700a87f8f4801520b2fa1;
    bytes32 private constant SP1_GATEWAY_RUNTIME_CODE_HASH =
        0x028169f823c247e78b55e899ff3e88d87587acc97a9bbbd67ebd58bcf15ef491;
    bytes4 private constant SP1_GROTH16_SELECTOR = 0x4388a21c;
    bytes private constant ROUTE_BINDING_DOMAIN = "postfiat.vault_bridge.route_binding.v1";

    error WrongChain(uint256 actual);
    error WrongLength(bytes32 field, uint256 actual, uint256 expected);
    error WrongValue(bytes32 field);
    error WrongCodeHash(bytes32 field, bytes32 actual, bytes32 expected);
    error WrongFactoryState();
    error WrongSp1Route(address verifier, bool frozen);
    error UintOverflow(bytes32 field, uint256 value);
    error PairReadbackFailed(bytes32 field);

    event ArcPfUsdcPairDeployed(
        address indexed finalityVerifier,
        address indexed anchor,
        address indexed vault,
        bytes32 routeBinding,
        bytes32 routeProfileHashCommitment,
        bytes32 vaultRuntimeCodeHash,
        bytes32 tokenRuntimeCodeHash
    );

    struct PairContext {
        uint256 privateKey;
        address owner;
        ArcPfUsdcDeploymentFactory factory;
        address predictedAnchor;
        address predictedVault;
        bytes32 tokenRuntimeCodeHash;
        bytes32 vaultRuntimeCodeHash;
        bytes32 routeBinding;
    }

    function run()
        external
        returns (PFTLFinalityVerifierV1 finalityVerifier, PfUsdcIngressAnchorV1 anchor, ERC20BridgeVaultV2 vault)
    {
        if (block.chainid != ARC_TESTNET_CHAIN_ID) revert WrongChain(block.chainid);
        PairContext memory context;
        context.privateKey = vm.envUint("PRIVATE_KEY");
        context.owner = vm.envAddress("PFUSDC_OWNER");
        if (context.owner == address(0)) revert WrongValue("owner");

        context.factory = ArcPfUsdcDeploymentFactory(vm.envAddress("ARC_PFUSDC_FACTORY"));
        if (context.factory.deployer() != vm.addr(context.privateKey) || context.factory.deployed()) {
            revert WrongFactoryState();
        }
        context.predictedAnchor = context.factory.predictedAnchor();
        context.predictedVault = context.factory.predictedVault();

        context.tokenRuntimeCodeHash = ARC_TESTNET_USDC.codehash;
        if (context.tokenRuntimeCodeHash == bytes32(0)) revert WrongValue("token_code_hash");
        address gatewayAddress = _validatedGateway();
        context.vaultRuntimeCodeHash = vm.envBytes32("ARC_VAULT_RUNTIME_CODE_HASH");
        if (context.vaultRuntimeCodeHash == bytes32(0)) revert WrongValue("vault_code_hash");
        PFTLFinalityVerifierV1.Config memory config;
        (config, context.routeBinding) =
            _ceremonyConfig(gatewayAddress, context.vaultRuntimeCodeHash, context.tokenRuntimeCodeHash);

        (finalityVerifier, anchor, vault) = _deploy(context, config);

        if (address(anchor) != context.predictedAnchor) revert PairReadbackFailed("anchor_address");
        if (address(vault) != context.predictedVault) revert PairReadbackFailed("vault_address");
        if (address(vault).codehash != context.vaultRuntimeCodeHash) {
            revert PairReadbackFailed("vault_code_hash");
        }
        if (!anchor.directIngress() || !vault.directIngress()) revert PairReadbackFailed("direct_mode");
        if (anchor.governedRouteBinding() != context.routeBinding) {
            revert PairReadbackFailed("route_binding");
        }
        if (address(vault.finalityVerifier()) != address(finalityVerifier)) {
            revert PairReadbackFailed("finality_verifier");
        }
        if (finalityVerifier.programVKey() != EGRESS_PROGRAM_VKEY) {
            revert PairReadbackFailed("program_vkey");
        }

        emit ArcPfUsdcPairDeployed(
            address(finalityVerifier),
            address(anchor),
            address(vault),
            context.routeBinding,
            config.routeProfileHashCommitment,
            context.vaultRuntimeCodeHash,
            context.tokenRuntimeCodeHash
        );
    }

    function _deploy(PairContext memory context, PFTLFinalityVerifierV1.Config memory config)
        private
        returns (PFTLFinalityVerifierV1 finalityVerifier, PfUsdcIngressAnchorV1 anchor, ERC20BridgeVaultV2 vault)
    {
        vm.startBroadcast(context.privateKey);
        finalityVerifier = new PFTLFinalityVerifierV1(config);
        (anchor, vault) = context.factory
            .deploy(
                IERC20BridgeTokenV2(ARC_TESTNET_USDC),
                IPFTLFinalityVerifierV1(address(finalityVerifier)),
                context.routeBinding,
                context.owner
            );
        vm.stopBroadcast();
    }

    function _validatedGateway() private returns (address gatewayAddress) {
        gatewayAddress = vm.envAddress("ARC_SP1_GATEWAY");
        _requireCodeHash("sp1_gateway", gatewayAddress, SP1_GATEWAY_RUNTIME_CODE_HASH);
        (address routedVerifier, bool frozen) = IArcSp1Gateway(gatewayAddress).routes(SP1_GROTH16_SELECTOR);
        if (routedVerifier == address(0) || frozen) revert WrongSp1Route(routedVerifier, frozen);
        _requireCodeHash("sp1_verifier", routedVerifier, SP1_VERIFIER_RUNTIME_CODE_HASH);
    }

    function _ceremonyConfig(address gatewayAddress, bytes32 vaultRuntimeCodeHash, bytes32 tokenRuntimeCodeHash)
        private
        returns (PFTLFinalityVerifierV1.Config memory config, bytes32 routeBinding)
    {
        config.sp1Verifier = ISP1Verifier(gatewayAddress);
        config.programVKey = EGRESS_PROGRAM_VKEY;
        config.pftlChainIdHash = keccak256(bytes(vm.envString("PFTL_CHAIN_ID")));
        config.pftlProtocolVersion = 1;
        config.arbitrumChainId = uint64(ARC_TESTNET_CHAIN_ID);
        config.vaultRuntimeCodeHash = vaultRuntimeCodeHash;
        config.token = ARC_TESTNET_USDC;
        config.tokenRuntimeCodeHash = tokenRuntimeCodeHash;
        config.maxProofBytes = 4_096;
        config.maxPublicValuesBytes = 16_384;

        {
            bytes memory value = _envBytes48("PFTL_GENESIS_HASH");
            config.pftlGenesisHashCommitment = keccak256(value);
        }
        {
            bytes memory value = _envBytes48("PFUSDC_ASSET_ID");
            config.assetIdCommitment = keccak256(value);
        }
        {
            bytes memory value = _envBytes48("PFTL_INITIAL_CHECKPOINT_BLOCK_ID");
            config.initialCheckpointCommitment = keccak256(value);
        }
        {
            bytes memory value = _envBytes48("PFTL_INITIAL_COMMITTEE_ROOT");
            config.initialCommitteeRootCommitment = keccak256(value);
        }
        {
            bytes memory routeProfileHash = _envBytes48("PFTL_ROUTE_PROFILE_HASH");
            config.routeProfileHashCommitment = keccak256(routeProfileHash);
            config.routeEpoch = _envUint64("PFTL_ROUTE_EPOCH");
            if (config.routeEpoch == 0 || config.routeEpoch > type(uint32).max) {
                revert UintOverflow("route_epoch", config.routeEpoch);
            }
            routeBinding = keccak256(
                abi.encodePacked(ROUTE_BINDING_DOMAIN, bytes1(0), routeProfileHash, uint32(config.routeEpoch))
            );
        }
        config.initialFinalizedHeight = _envUint64("PFTL_INITIAL_FINALIZED_HEIGHT");
        if (config.pftlChainIdHash == keccak256(bytes("")) || config.initialFinalizedHeight == 0) {
            revert WrongValue("pftl_ceremony");
        }
    }

    function _envBytes48(string memory name) private returns (bytes memory value) {
        value = vm.envBytes(name);
        if (value.length != 48) revert WrongLength(bytes32(bytes(name)), value.length, 48);
    }

    function _envUint64(string memory name) private returns (uint64 value) {
        uint256 raw = vm.envUint(name);
        if (raw > type(uint64).max) revert UintOverflow(bytes32(bytes(name)), raw);
        // forge-lint: disable-next-line(unsafe-typecast)
        value = uint64(raw);
    }

    function _requireCodeHash(bytes32 field, address target, bytes32 expected) private view {
        bytes32 actual = target.codehash;
        if (actual != expected) revert WrongCodeHash(field, actual, expected);
    }
}
