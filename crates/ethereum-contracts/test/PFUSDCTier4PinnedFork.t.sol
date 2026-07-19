// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    ERC20BridgeVaultV2,
    IArbSysPfUsdcV1,
    IERC20BridgeTokenV2,
    IPFTLFinalityVerifierV1
} from "../src/ERC20BridgeVaultV2.sol";
import {PFTLFinalityVerifierV1, ISP1Verifier} from "../src/PFTLFinalityVerifierV1.sol";
import {IArbitrumBridgeV1, PfUsdcIngressAnchorV1} from "../src/PfUsdcIngressAnchorV1.sol";

interface Tier4ForkVm {
    function createSelectFork(string calldata url, uint256 blockNumber) external returns (uint256);
    function envString(string calldata key) external returns (string memory);
}

contract PFUSDCTier4PinnedForkTest {
    Tier4ForkVm private constant vm = Tier4ForkVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant ARBITRUM_SEPOLIA_BLOCK = 288_769_892;
    uint256 private constant ETHEREUM_SEPOLIA_BLOCK = 11_298_882;
    uint256 private constant ARBITRUM_SEPOLIA_CHAIN_ID = 421_614;
    uint256 private constant ETHEREUM_SEPOLIA_CHAIN_ID = 11_155_111;

    address private constant TOKEN = address(bytes20(hex"75faf114eafb1bdbe2f0316df893fd58ce46aa4d"));
    address private constant SP1_VERIFIER = address(bytes20(hex"3b6041173b80e77f038f3f2c0f9744f04837185e"));
    address private constant ARB_SYS = address(bytes20(hex"0000000000000000000000000000000000000064"));
    address private constant ETHEREUM_BRIDGE = address(bytes20(hex"38f918d0e9f1b721edaa41302e399fa1b79333a9"));
    address private constant FROZEN_VAULT = address(bytes20(hex"a796dc3c9308f9c855a0659153b7afc2006cf27b"));
    address private constant FROZEN_ANCHOR = address(bytes20(hex"89ec019b4aa5423b8d96152a502a0db52cf48164"));
    address private constant FROZEN_OWNER = address(bytes20(hex"1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"));

    bytes32 private constant TOKEN_RUNTIME_HASH = 0x9a736af6aac290d9196883e8686fc1d127ff657ca534fe4b88d6d40dc0bc6750;
    bytes32 private constant SP1_RUNTIME_HASH = 0xdcba737cf430260fdbc8010a56d97a9f29e64465155819e74d75da8f95eb24ed;
    bytes32 private constant ETHEREUM_BRIDGE_RUNTIME_HASH =
        0x8736329b580cfc0c0c39ee6700515e0bc51652afb614640db9e34a5d784933e8;
    bytes32 private constant FINALITY_RUNTIME_HASH = 0x8dd7e23c7d42a104fc91893cfc93184c0d2d2a7e2b2115574c9a048e82fdb781;
    bytes32 private constant VAULT_RUNTIME_HASH = 0xc53ec5dad1757e65df90675446ee1f02bcadafbde12a4df4ccb396f7a98b9812;
    bytes32 private constant ANCHOR_RUNTIME_HASH = 0x3a5e3f49d40d340dd996975d29bb4a17669ab3a8f32f1dc1d0c13e1889825fc0;
    bytes32 private constant ROUTE_BINDING = 0xd072739d73648a6b3bf853ab284da9072584ad83605a16a66de4748b110b795c;

    function testPinnedArbitrumSepoliaDependenciesAndFrozenRuntime() public {
        vm.createSelectFork(vm.envString("ARBITRUM_SEPOLIA_RPC_URL"), ARBITRUM_SEPOLIA_BLOCK);
        require(block.chainid == ARBITRUM_SEPOLIA_CHAIN_ID, "wrong Arbitrum Sepolia chain");
        require(TOKEN.codehash == TOKEN_RUNTIME_HASH, "canonical USDC runtime hash drift");
        require(SP1_VERIFIER.codehash == SP1_RUNTIME_HASH, "SP1 verifier runtime hash drift");

        PFTLFinalityVerifierV1 verifier = new PFTLFinalityVerifierV1(
            PFTLFinalityVerifierV1.Config({
                sp1Verifier: ISP1Verifier(SP1_VERIFIER),
                programVKey: 0x00eaaf9372917c3edf9d6fdf70ff64ae08ba25e13cb1e2b2ab7b6e9585d50cd4,
                pftlChainIdHash: 0xe61f64cfb4299057c29b1c1dbf031c81beeb1b016878f4a087824eaa14c2cc00,
                pftlGenesisHashCommitment: 0x396aceff4e14037223cc2781a3197c8eef78f44903b4e00d2e5441abec789418,
                pftlProtocolVersion: 1,
                routeProfileHashCommitment: 0x537f79e1e46b8f0987ef20d6133a4babf6a7552ddf359773fadfc9b7c7960356,
                routeEpoch: 1,
                assetIdCommitment: 0x24e5fe4c65497dff08ec93064e5eb76e23c61992e7ec2c946136334a28eab403,
                arbitrumChainId: uint64(ARBITRUM_SEPOLIA_CHAIN_ID),
                vaultRuntimeCodeHash: VAULT_RUNTIME_HASH,
                token: TOKEN,
                tokenRuntimeCodeHash: TOKEN_RUNTIME_HASH,
                maxProofBytes: 4096,
                maxPublicValuesBytes: 16384,
                initialCheckpointCommitment: 0xc5d6128ed1ae4b32030df55bccd71015cb7ac12591218160f9529f4ccde5a49b,
                initialFinalizedHeight: 1,
                initialCommitteeRootCommitment: 0xaaffb2ebf63f1201c65bef3f71537474225cedafdd30dfe0f3a2bfe5887bdc11
            })
        );
        require(address(verifier).codehash == FINALITY_RUNTIME_HASH, "finality verifier runtime hash drift");
        require(
            verifier.programVKey() == 0x00eaaf9372917c3edf9d6fdf70ff64ae08ba25e13cb1e2b2ab7b6e9585d50cd4,
            "vkey readback drift"
        );

        ERC20BridgeVaultV2 vault = new ERC20BridgeVaultV2(
            IERC20BridgeTokenV2(TOKEN),
            IPFTLFinalityVerifierV1(address(verifier)),
            TOKEN_RUNTIME_HASH,
            IArbSysPfUsdcV1(ARB_SYS),
            FROZEN_ANCHOR,
            FROZEN_OWNER
        );
        require(address(vault).codehash == VAULT_RUNTIME_HASH, "vault runtime hash drift");
        require(address(vault.token()) == TOKEN, "vault token readback drift");
        require(vault.ingressAnchor() == FROZEN_ANCHOR, "vault anchor readback drift");
        require(vault.owner() == FROZEN_OWNER, "vault owner readback drift");
    }

    function testPinnedEthereumSepoliaDependencyAndFrozenAnchorRuntime() public {
        vm.createSelectFork(vm.envString("ETHEREUM_SEPOLIA_RPC_URL"), ETHEREUM_SEPOLIA_BLOCK);
        require(block.chainid == ETHEREUM_SEPOLIA_CHAIN_ID, "wrong Ethereum Sepolia chain");
        require(ETHEREUM_BRIDGE.codehash == ETHEREUM_BRIDGE_RUNTIME_HASH, "Arbitrum bridge runtime hash drift");

        PfUsdcIngressAnchorV1 anchor = new PfUsdcIngressAnchorV1(
            IArbitrumBridgeV1(ETHEREUM_BRIDGE), FROZEN_VAULT, TOKEN, ARBITRUM_SEPOLIA_CHAIN_ID, ROUTE_BINDING
        );
        require(address(anchor).codehash == ANCHOR_RUNTIME_HASH, "ingress anchor runtime hash drift");
        require(anchor.l2Vault() == FROZEN_VAULT, "anchor vault readback drift");
        require(anchor.l2Token() == TOKEN, "anchor token readback drift");
        require(anchor.l2ChainId() == ARBITRUM_SEPOLIA_CHAIN_ID, "anchor chain readback drift");
        require(anchor.governedRouteBinding() == ROUTE_BINDING, "anchor route binding readback drift");
    }
}
