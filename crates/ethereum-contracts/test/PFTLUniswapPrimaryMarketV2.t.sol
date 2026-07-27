// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    IA666WrappedToken,
    IPFTLReceiptFinalityVerifierV1,
    PFTLUniswapPrimaryMarketV2
} from "../src/PFTLUniswapPrimaryMarketV2.sol";

interface VmV2 {
    function expectRevert(bytes4 selector) external;
    function expectRevert(bytes calldata revertData) external;
    function prank(address sender) external;
}

contract MockA666Token is IA666WrappedToken {
    uint8 public constant decimals = 6;
    mapping(address => uint256) public balanceOf;
    uint256 public totalSupply;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }

    function burnFromBridge(address from, uint256 amount) external {
        balanceOf[from] -= amount;
        totalSupply -= amount;
    }
}

contract MockReceiptFinalityVerifier is IPFTLReceiptFinalityVerifierV1 {
    bytes32 internal constant TRUSTLESS = keccak256("TRUSTLESS_FINALITY");
    mapping(bytes32 => bool) public accepted;

    function routeTrustClass() external pure returns (bytes32) {
        return TRUSTLESS;
    }

    function setAccepted(bytes32 packetDigest, bool value) external {
        accepted[packetDigest] = value;
    }

    function isReceiptAccepted(
        bytes calldata,
        bytes calldata,
        bytes calldata,
        bytes32 routeTrustClass_,
        bytes32 packetDigest
    ) external view returns (bool) {
        return routeTrustClass_ == TRUSTLESS && accepted[packetDigest];
    }
}

contract PFTLUniswapPrimaryMarketV2Test {
    VmV2 internal constant vm = VmV2(address(uint160(uint256(keccak256("hevm cheat code")))));

    MockA666Token internal token;
    MockReceiptFinalityVerifier internal verifier;
    PFTLUniswapPrimaryMarketV2 internal controller;

    bytes internal routeDigest = bytes.concat(bytes32(uint256(1)), bytes16(uint128(2)));
    bytes internal settlementId = bytes.concat(bytes32(uint256(3)), bytes16(uint128(4)));
    bytes internal nativeId = bytes.concat(bytes32(uint256(5)), bytes16(uint128(6)));
    bytes internal reserveHash = bytes.concat(bytes32(uint256(7)), bytes16(uint128(8)));

    function setUp() public {
        token = new MockA666Token();
        verifier = new MockReceiptFinalityVerifier();
        controller = new PFTLUniswapPrimaryMarketV2(
            token,
            verifier,
            PFTLUniswapPrimaryMarketV2.Config({
                destinationChainId: block.chainid,
                settlementAssetId: settlementId,
                nativeNavAssetId: nativeId,
                uniswapPoolId: bytes32(uint256(10)),
                routeSupplyCapAtoms: 2_000_000e6,
                packetNotionalCapAtoms: 250_000e6,
                governance: address(this)
            })
        );
        require(controller.mintPaused(), "controller must deploy paused");
        controller.setMintPaused(false);
    }

    function testPermissionlessMintConsumesProofOnce() public {
        PFTLUniswapPrimaryMarketV2.MintPacket memory packet = _packet(bytes32(uint256(11)));
        bytes32 digest = controller.packetDigest(packet);
        verifier.setAccepted(digest, true);

        address relayer = address(0xB0B);
        vm.prank(relayer);
        controller.consumeMintOnly(packet);

        require(token.balanceOf(packet.ethereumRecipient) == 250_000e6, "recipient mint");
        require(controller.outstandingMintedAtoms() == 250_000e6, "outstanding");
        vm.expectRevert(
            abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.PacketReplay.selector, digest)
        );
        controller.consumeMintOnly(packet);
    }

    function testProofValidatedPolicyEpochCanRotateWithoutControllerRedeploy() public {
        PFTLUniswapPrimaryMarketV2.MintPacket memory packet = _packet(bytes32(uint256(21)));
        packet.routeConfigDigest = _filled(0x42);
        packet.pricingReservePacketHash = _filled(0x43);
        packet.policyHashCommitment = bytes32(uint256(44));
        packet.routeEpoch = 2;
        packet.pricingNavEpoch = 13;
        bytes32 digest = controller.packetDigest(packet);
        verifier.setAccepted(digest, true);

        controller.consumeMintOnly(packet);

        require(token.balanceOf(packet.ethereumRecipient) == 250_000e6, "rotated recipient mint");
    }

    function testConstructorRejectsNonCanonicalA666Caps() public {
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLUniswapPrimaryMarketV2.PacketBindingMismatch.selector,
                bytes32("a666_launch_parameters")
            )
        );
        new PFTLUniswapPrimaryMarketV2(
            token,
            verifier,
            PFTLUniswapPrimaryMarketV2.Config({
                destinationChainId: block.chainid,
                settlementAssetId: settlementId,
                nativeNavAssetId: nativeId,
                uniswapPoolId: bytes32(uint256(10)),
                routeSupplyCapAtoms: 2_000_000e6 + 1,
                packetNotionalCapAtoms: 250_000e6,
                governance: address(this)
            })
        );
    }

    function testUnprovenAndOversizedPacketsFailClosed() public {
        PFTLUniswapPrimaryMarketV2.MintPacket memory packet = _packet(bytes32(uint256(12)));
        bytes32 digest = controller.packetDigest(packet);
        vm.expectRevert(
            abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.ReceiptNotAccepted.selector, digest)
        );
        controller.consumeMintOnly(packet);

        packet.mintAmountAtoms = 250_000e6 + 1;
        vm.expectRevert(
            abi.encodeWithSelector(
                PFTLUniswapPrimaryMarketV2.PacketNotionalCapExceeded.selector,
                250_000e6 + 1,
                250_000e6
            )
        );
        controller.consumeMintOnly(packet);
    }

    function testLaunchControllerHasNoMintAndSwapEntryPoint() public {
        (bool ok,) = address(controller).call(
            abi.encodeWithSignature("consumeMintAndSwap(bytes,bytes)", bytes("packet"), bytes("swap"))
        );
        require(!ok, "mint-and-swap must be absent");
    }

    function testGovernancePauseBlocksMintButNotReturnBurn() public {
        PFTLUniswapPrimaryMarketV2.MintPacket memory packet = _packet(bytes32(uint256(31)));
        bytes32 digest = controller.packetDigest(packet);
        verifier.setAccepted(digest, true);
        controller.consumeMintOnly(packet);
        controller.setMintPaused(true);
        vm.expectRevert(PFTLUniswapPrimaryMarketV2.MintingPaused.selector);
        controller.consumeMintOnly(_packet(bytes32(uint256(32))));

        vm.prank(packet.ethereumRecipient);
        controller.burnForPftlReturn(
            100e6,
            "bob-pftl",
            nativeId,
            bytes32(uint256(33))
        );
        require(controller.outstandingMintedAtoms() == 249_900e6, "return burn blocked");
    }

    function testPacketDigestMatchesRustVectorAndExcludesReceiptFields() public view {
        PFTLUniswapPrimaryMarketV2.MintPacket memory packet =
            PFTLUniswapPrimaryMarketV2.MintPacket({
                routeConfigDigest: _filled(0x11),
                sourcePacketHash: _filled(0x22),
                reservationId: _filled(0xaa),
                sourceReceiptHash: _filled(0x33),
                sourceReceiptRoot: _filled(0x44),
                settlementAssetId: _filled(0x55),
                nativeNavAssetId: _filled(0x66),
                pricingReservePacketHash: _filled(0x77),
                policyHashCommitment: bytes32(uint256(type(uint256).max / 0xff * 0x88)),
                routeEpoch: 7,
                pricingNavEpoch: 9,
                deadline: 1_800_000_000,
                nonce: bytes32(uint256(type(uint256).max / 0xff * 0x99)),
                destinationChainId: 1,
                destinationController: 0x1111111111111111111111111111111111111111,
                wrappedToken: 0x2222222222222222222222222222222222222222,
                ethereumRecipient: 0x3333333333333333333333333333333333333333,
                mintAmountAtoms: 250_000e6,
                settlementValueAtoms: 251_250e6
            });
        bytes32 expected =
            hex"a8bfc40472aed2c1cc514509249c8d0477962a41f262545dd53e24550d8c67c3";
        require(controller.packetDigest(packet) == expected, "Rust/Solidity digest mismatch");
        packet.sourceReceiptHash = _filled(0xaa);
        packet.sourceReceiptRoot = _filled(0xbb);
        require(controller.packetDigest(packet) == expected, "receipt must bind outside digest");
        packet.reservationId = _filled(0xab);
        require(controller.packetDigest(packet) != expected, "reservation must bind inside digest");
    }

    function _packet(bytes32 nonce)
        private
        view
        returns (PFTLUniswapPrimaryMarketV2.MintPacket memory packet)
    {
        packet = PFTLUniswapPrimaryMarketV2.MintPacket({
            routeConfigDigest: routeDigest,
            sourcePacketHash: bytes.concat(bytes32(uint256(13)), bytes16(uint128(14))),
            reservationId: bytes.concat(bytes32(uint256(19)), bytes16(uint128(20))),
            sourceReceiptHash: bytes.concat(bytes32(uint256(15)), bytes16(uint128(16))),
            sourceReceiptRoot: bytes.concat(bytes32(uint256(17)), bytes16(uint128(18))),
            settlementAssetId: settlementId,
            nativeNavAssetId: nativeId,
            pricingReservePacketHash: reserveHash,
            policyHashCommitment: bytes32(uint256(9)),
            routeEpoch: 1,
            pricingNavEpoch: 12,
            deadline: uint64(block.timestamp + 1 hours),
            nonce: nonce,
            destinationChainId: block.chainid,
            destinationController: address(controller),
            wrappedToken: address(token),
            ethereumRecipient: address(0xA666),
            mintAmountAtoms: 250_000e6,
            settlementValueAtoms: 251_250e6
        });
    }

    function _filled(bytes1 value) private pure returns (bytes memory output) {
        output = new bytes(48);
        for (uint256 i = 0; i < output.length; i++) output[i] = value;
    }
}
