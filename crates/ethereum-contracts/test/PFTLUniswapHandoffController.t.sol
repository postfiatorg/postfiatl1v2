// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    ControlledPFTLReceiptVerifier,
    IExactInputRouter,
    IVenueMintableToken,
    OptimisticPFTLReceiptVerifier,
    PacketReplayRegistry,
    PFTLUniswapHandoffController,
    ThresholdPFTLReceiptVerifier,
    UniswapSettlementAdapter,
    WrappedVenueNAVCoin
} from "../src/PFTLUniswapHandoffController.sol";

interface HandoffVm {
    function warp(uint256 timestamp) external;
    function chainId(uint256 chain_id) external;
    function prank(address sender) external;
    function deal(address account, uint256 newBalance) external;
    function expectEmit(bool checkTopic1, bool checkTopic2, bool checkTopic3, bool checkData) external;
    function addr(uint256 private_key) external returns (address);
    function sign(uint256 private_key, bytes32 digest) external returns (uint8 v, bytes32 r, bytes32 s);
}

contract PFTLUniswapHandoffControllerTest {
    HandoffVm private constant vm = HandoffVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    WrappedVenueNAVCoin private wrapped;
    HandoffMockToken private usdc;
    HandoffMockRouter private router;
    UniswapSettlementAdapter private adapter;
    ControlledPFTLReceiptVerifier private verifier;
    PacketReplayRegistry private replay_registry;
    PFTLUniswapHandoffController private controller;

    bytes32 private constant POOL_ID = bytes32(uint256(0x4444));
    bytes32 private constant WALLET_HASH = bytes32(uint256(0x6666));
    bytes32 private constant NONCE = bytes32(uint256(0x7777));
    bytes32 private constant TRUST_CLASS_CONTROLLED = keccak256("CONTROLLED");
    bytes32 private constant TRUST_CLASS_OPTIMISTIC = keccak256("OPTIMISTIC");
    bytes32 private constant TRUST_CLASS_TRUSTLESS_FINALITY = keccak256("TRUSTLESS_FINALITY");
    bytes32 private constant TRUST_CLASS_BFT_CHECKPOINT = keccak256("BFT_CHECKPOINT");
    bytes32 private constant TRUST_CLASS_DISABLED = keccak256("DISABLED");
    address private constant RECIPIENT = address(0xBEEF);
    address private constant OPTIMISTIC_POSTER = address(0xABCD);
    address private constant OPTIMISTIC_CHALLENGER = address(0xCAFE);
    uint256 private constant OPTIMISTIC_POSTER_BOND = 1 ether;
    uint256 private constant OPTIMISTIC_CHALLENGER_BOND = 2 ether;
    uint64 private constant OPTIMISTIC_CHALLENGE_WINDOW = 100;
    uint64 private constant OPTIMISTIC_CHALLENGE_RESOLUTION_WINDOW = 50;
    bytes32 private constant OPTIMISTIC_CHALLENGE_EVIDENCE_HASH =
        keccak256("postfiat.optimistic.challenge.evidence.test");
    string private constant PFTL_RECIPIENT = "pf124071fd53a12ca4556b7aa1f5ec98b585e73468";

    bytes private constant SWAP_DATA = hex"01020304";

    event PacketConsumed(
        bytes32 indexed packet_digest,
        bytes32 indexed source_packet_commitment,
        address indexed recipient,
        bytes32 route_config_commitment,
        bytes32 source_receipt_commitment,
        bytes32 route_trust_class,
        uint256 mint_amount_atoms,
        uint256 settlement_amount_atoms
    );
    event PacketCancelled(
        bytes32 indexed packet_digest,
        bytes32 indexed source_packet_commitment,
        bytes32 indexed source_receipt_commitment,
        uint64 deadline,
        uint64 cancelled_at
    );
    event PausedSet(bool paused);

    receive() external payable {}

    function setUp() public {
        wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        usdc = new HandoffMockToken();
        router = new HandoffMockRouter(POOL_ID);
        router.setAmountOut(95);
        verifier = new ControlledPFTLReceiptVerifier(address(this), TRUST_CLASS_CONTROLLED);
        replay_registry = new PacketReplayRegistry(address(this));
        controller = _controller(TRUST_CLASS_CONTROLLED);
        wrapped.setController(address(controller));
        wrapped.lockController();
        vm.warp(1_000);
    }

    function testGate1SidecarExportVectorConsumesMintOnly() public {
        vm.chainId(1);
        WrappedVenueNAVCoin vector_wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        ControlledPFTLReceiptVerifier vector_verifier =
            new ControlledPFTLReceiptVerifier(address(this), TRUST_CLASS_CONTROLLED);
        PFTLUniswapHandoffController.RouteConfig memory config;
        config.initial_owner = address(this);
        config.destination_chain_id = block.chainid;
        config.route_config_digest =
        hex"2b7c9e4ff093219998ce15e9d40163e7c42968165474fa35e418813482785a2aba2fe3c94d4e489e1cffb6dfd09c60d6";
        config.route_trust_class = TRUST_CLASS_CONTROLLED;
        config.settlement_asset_id =
        hex"888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888";
        config.native_nav_asset_id =
        hex"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        config.pricing_reserve_packet_hash =
        hex"999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999";
        config.pricing_nav_epoch = 7;
        config.uniswap_pool_id = POOL_ID;
        config.route_supply_cap_atoms = 10_000_000;
        config.packet_notional_cap_atoms = 1_000_000;
        PacketReplayRegistry vector_replay_registry = new PacketReplayRegistry(address(this));
        config.replay_registry = address(vector_replay_registry);

        PFTLUniswapHandoffController vector_controller = new PFTLUniswapHandoffController(
            IVenueMintableToken(address(vector_wrapped)), IExactInputRouter(address(router)), vector_verifier, config
        );
        vector_replay_registry.setControllerAuthorization(address(vector_controller), true);
        vector_wrapped.setController(address(vector_controller));
        vector_wrapped.lockController();

        PFTLUniswapHandoffController.MintAndSwapPacket memory packet;
        packet.route_config_digest = config.route_config_digest;
        packet.source_packet_hash =
        hex"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        packet.source_receipt_hash =
        hex"ea07ef495216c0d3ee1c4d94d94b17ab5ea906f84bea7dbb98e6ad047aa9571569bf12e5b5c79f56447bf54a75b5ada2";
        packet.source_receipt_root =
        hex"d77256242815519edf4127cac1c6ed90914b629df6d66b80bf29e749d0fdd8cc330999cc9a001be9b8aad0483b480900";
        packet.destination_chain_id = block.chainid;
        packet.destination_bridge = address(vector_controller);
        packet.wrapped_navcoin_token = address(vector_wrapped);
        packet.source_wallet_hash = WALLET_HASH;
        packet.settlement_asset_id = config.settlement_asset_id;
        packet.native_nav_asset_id = config.native_nav_asset_id;
        packet.pricing_reserve_packet_hash = config.pricing_reserve_packet_hash;
        packet.uniswap_pool_id = POOL_ID;
        packet.swap_path_hash = bytes32(0);
        packet.ethereum_recipient = address(0x6666666666666666666666666666666666666666);
        packet.token_out = address(usdc);
        packet.settlement_amount_atoms = 40;
        packet.mint_amount_atoms = 40;
        packet.minimum_output_atoms = 1;
        packet.pricing_nav_epoch = 7;
        packet.deadline = 1_924_992_000;
        packet.nonce = bytes32(uint256(0xbbbb));

        bytes32 digest = vector_controller.packetDigest(packet);
        vector_verifier.setReceiptAcceptance(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest, true
        );

        bytes32 consumed = vector_controller.consumeMintOnly(packet);

        _assertEq(consumed, digest, "gate1 vector digest");
        _assertTrue(vector_controller.consumed_packet(digest), "gate1 vector consumed");
        _assertEq(
            vector_wrapped.balanceOf(address(0x6666666666666666666666666666666666666666)),
            40,
            "gate1 vector recipient wrapped"
        );
        _assertEq(vector_controller.total_minted_atoms(), 40, "gate1 vector minted");
    }

    function testConsumeMintOnlyMintsWrappedTokenAndRejectsReplay() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);

        bytes32 consumed = controller.consumeMintOnly(packet);

        _assertEq(consumed, digest, "digest");
        _assertTrue(controller.consumed_packet(digest), "consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "recipient wrapped");
        _assertEq(controller.total_minted_atoms(), 10, "total minted");
        _assertEq(controller.total_settlement_atoms(), 100, "total settlement");
        _expectMintOnlyRevert(packet);
    }

    function testExpiredCancellationAndConsumeAreMutuallyExclusive() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory cancelled = _packet(100, 10, 1_500, bytes32(0));
        bytes32 cancelled_digest = controller.packetDigest(cancelled);
        bytes32 source_packet_commitment = keccak256(cancelled.source_packet_hash);
        bytes32 source_receipt_commitment = _sourceReceiptReplayCommitment(cancelled);

        try controller.cancelExpiredPacket(cancelled) {
            revert("expected cancellation before deadline to revert");
        } catch {}
        _assertTrue(!controller.cancelled_packet(cancelled_digest), "premature cancellation absent");
        _assertTrue(!controller.consumed_packet(cancelled_digest), "premature consume absent");

        vm.warp(1_501);
        vm.expectEmit(true, true, true, true);
        emit PacketCancelled(
            cancelled_digest, source_packet_commitment, source_receipt_commitment, cancelled.deadline, 1_501
        );
        bytes32 cancellation = controller.cancelExpiredPacket(cancelled);

        _assertEq(cancellation, cancelled_digest, "cancelled digest");
        _assertTrue(controller.cancelled_packet(cancelled_digest), "cancelled status");
        _assertTrue(!controller.consumed_packet(cancelled_digest), "cancel is not consume");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "cancel does not mint");
        _expectMintOnlyRevert(cancelled);
        try controller.cancelExpiredPacket(cancelled) {
            revert("expected duplicate cancellation to revert");
        } catch {}

        PFTLUniswapHandoffController.MintAndSwapPacket memory consumed =
            _uniqueAcceptedPacket(0x81, 0x82, bytes32(uint256(0x8182)), 101, 11, 1_600, bytes32(0));
        bytes32 consumed_digest = controller.packetDigest(consumed);
        vm.warp(1_600);
        controller.consumeMintOnly(consumed);
        vm.warp(1_601);
        try controller.cancelExpiredPacket(consumed) {
            revert("expected cancellation after consumption to revert");
        } catch {}
        _assertTrue(controller.consumed_packet(consumed_digest), "consumed status");
        _assertTrue(!controller.cancelled_packet(consumed_digest), "consumed is not cancelled");
        _assertEq(wrapped.balanceOf(RECIPIENT), 11, "consume mints exactly once");
    }

    function testRouteTrustClassIsMachineReadableAndEmittedOnConsume() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);
        bytes32 source_commitment = keccak256(packet.source_packet_hash);
        bytes32 source_receipt_commitment = _sourceReceiptReplayCommitment(packet);

        _assertEq(controller.route_trust_class(), TRUST_CLASS_CONTROLLED, "controller trust class");
        _assertEq(controller.verifierTrustClass(), TRUST_CLASS_CONTROLLED, "verifier trust class");

        vm.expectEmit(true, true, true, true);
        emit PacketConsumed(
            digest,
            source_commitment,
            RECIPIENT,
            keccak256(packet.route_config_digest),
            source_receipt_commitment,
            TRUST_CLASS_CONTROLLED,
            packet.mint_amount_atoms,
            packet.settlement_amount_atoms
        );

        controller.consumeMintOnly(packet);
    }

    function testWrappedTokenRejectsDirectMintAndBurnsForReturn() public {
        try wrapped.mint(RECIPIENT, 1) {
            revert("expected direct mint revert");
        } catch {}

        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        controller.consumeMintOnly(packet);
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "recipient wrapped before burn");
        _assertEq(wrapped.totalSupply(), 10, "supply before burn");

        bytes32 nonce = bytes32(uint256(0x9999));
        vm.prank(RECIPIENT);
        bytes32 return_burn_id = controller.burnForPftlReturn(4, PFTL_RECIPIENT, _pftlBytes(0x33), nonce);

        _assertTrue(return_burn_id != bytes32(0), "return id");
        _assertEq(wrapped.balanceOf(RECIPIENT), 6, "recipient wrapped after burn");
        _assertEq(wrapped.totalSupply(), 6, "supply after burn");
        _assertEq(controller.total_return_burned_atoms(), 4, "return burned total");
        _assertTrue(controller.consumed_return_nonce(nonce), "return nonce consumed");

        vm.prank(RECIPIENT);
        _expectReturnBurnRevert(1, PFTL_RECIPIENT, _pftlBytes(0x33), nonce);

        vm.prank(RECIPIENT);
        _expectReturnBurnRevert(1, PFTL_RECIPIENT, _pftlBytes(0x44), bytes32(uint256(0x999A)));
    }

    function testWrappedTokenZeroTransfersAndControllerLockDecision() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        controller.consumeMintOnly(packet);
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "recipient wrapped before zero transfer");

        vm.prank(RECIPIENT);
        _assertTrue(wrapped.transfer(address(0xCAFE), 0), "zero transfer succeeds");
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "recipient unchanged");
        _assertEq(wrapped.balanceOf(address(0xCAFE)), 0, "zero recipient unchanged");

        _assertTrue(wrapped.transferFrom(RECIPIENT, address(0xCAFE), 0), "zero transferFrom succeeds");
        _assertEq(wrapped.allowance(RECIPIENT, address(this)), 0, "zero allowance unchanged");

        try wrapped.setController(address(0xCAFE)) {
            revert("expected locked controller revert");
        } catch {}
        _assertTrue(wrapped.controller() == address(controller), "controller remains locked");
    }

    function testRouteSupplyCapTracksNetOutstandingAfterReturnBurn() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory fill_cap =
            _uniqueAcceptedPacket(0x40, 0x41, bytes32(uint256(0x4041)), 100, 10_000, 1_500, bytes32(0));
        controller.consumeMintOnly(fill_cap);
        _assertEq(controller.total_minted_atoms(), 10_000, "lifetime minted at cap");
        _assertEq(controller.outstanding_minted_atoms(), 10_000, "outstanding at cap");

        PFTLUniswapHandoffController.MintAndSwapPacket memory over_cap =
            _uniqueAcceptedPacket(0x42, 0x43, bytes32(uint256(0x4243)), 100, 1, 1_500, bytes32(0));
        _expectMintOnlyRevert(over_cap);

        vm.prank(RECIPIENT);
        controller.burnForPftlReturn(4, PFTL_RECIPIENT, _pftlBytes(0x33), bytes32(uint256(0x4444)));
        _assertEq(controller.total_minted_atoms(), 10_000, "lifetime minted unchanged after burn");
        _assertEq(controller.total_return_burned_atoms(), 4, "return burn reduces exposure");
        _assertEq(controller.outstanding_minted_atoms(), 9_996, "outstanding reduced after burn");

        PFTLUniswapHandoffController.MintAndSwapPacket memory refill =
            _uniqueAcceptedPacket(0x44, 0x45, bytes32(uint256(0x4445)), 100, 4, 1_500, bytes32(0));
        controller.consumeMintOnly(refill);
        _assertEq(controller.total_minted_atoms(), 10_004, "lifetime minted remains audit counter");
        _assertEq(controller.outstanding_minted_atoms(), 10_000, "outstanding refilled to cap");

        PFTLUniswapHandoffController.MintAndSwapPacket memory still_over_cap =
            _uniqueAcceptedPacket(0x46, 0x47, bytes32(uint256(0x4647)), 100, 1, 1_500, bytes32(0));
        _expectMintOnlyRevert(still_over_cap);
    }

    function testSetPausedIsOwnerOnlyAndEmits() public {
        vm.prank(address(0xBADD));
        try controller.setPaused(true) {
            revert("expected non-owner pause revert");
        } catch {}
        _assertTrue(!controller.paused(), "pause unchanged");

        vm.expectEmit(false, false, false, true);
        emit PausedSet(true);
        controller.setPaused(true);
        _assertTrue(controller.paused(), "paused");

        vm.expectEmit(false, false, false, true);
        emit PausedSet(false);
        controller.setPaused(false);
        _assertTrue(!controller.paused(), "unpaused");
    }

    function testPauseBlocksInboundConsumesButAllowsReturnBurn() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        controller.consumeMintOnly(packet);
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "recipient wrapped before pause");

        controller.setPaused(true);

        PFTLUniswapHandoffController.MintAndSwapPacket memory paused_mint = _packet(101, 11, 1_500, bytes32(0));
        _expectMintOnlyRevert(paused_mint);

        PFTLUniswapHandoffController.MintAndSwapPacket memory paused_swap =
            _packet(102, 12, 1_500, keccak256(SWAP_DATA));
        _expectSwapRevert(paused_swap, SWAP_DATA);

        bytes32 nonce = bytes32(uint256(0xA001));
        vm.prank(RECIPIENT);
        bytes32 return_burn_id = controller.burnForPftlReturn(4, PFTL_RECIPIENT, _pftlBytes(0x33), nonce);

        _assertTrue(return_burn_id != bytes32(0), "return burn id");
        _assertEq(wrapped.balanceOf(RECIPIENT), 6, "recipient wrapped after paused return burn");
        _assertEq(wrapped.totalSupply(), 6, "supply after paused return burn");
        _assertEq(controller.total_return_burned_atoms(), 4, "paused return burned total");
    }

    function testSourcePacketHashRejectsMutatedReplay() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        bytes memory source_packet_hash = packet.source_packet_hash;
        bytes32 source_commitment = keccak256(source_packet_hash);

        controller.consumeMintOnly(packet);

        PFTLUniswapHandoffController.MintAndSwapPacket memory mutated = _packet(101, 11, 1_500, bytes32(0));
        mutated.source_packet_hash = source_packet_hash;
        _expectMintOnlyRevert(mutated);
        _assertTrue(controller.consumed_source_packet(source_commitment), "source consumed");
    }

    function testSourceReceiptHashRejectsAcceptedMutatedReplay() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        bytes32 receipt_commitment = _sourceReceiptReplayCommitment(packet);

        controller.consumeMintOnly(packet);

        PFTLUniswapHandoffController.MintAndSwapPacket memory mutated =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        mutated.source_packet_hash = _pftlBytes(0x99);
        mutated.nonce = bytes32(uint256(0x9998));
        bytes32 mutated_digest = controller.packetDigest(mutated);
        verifier.setReceiptAcceptance(
            mutated.source_receipt_root, mutated.source_receipt_hash, mutated.route_config_digest, mutated_digest, true
        );
        _expectMintOnlyRevert(mutated);

        _assertTrue(controller.consumed_source_receipt(receipt_commitment), "receipt consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "recipient wrapped unchanged");
        _assertEq(controller.total_minted_atoms(), 10, "total minted unchanged");
    }

    function testReplayRegistrySurvivesControllerRedeploy() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);
        controller.consumeMintOnly(packet);
        _assertTrue(replay_registry.consumed_packet(digest), "packet replay stored");

        bytes32 return_nonce = bytes32(uint256(0xB001));
        vm.prank(RECIPIENT);
        controller.burnForPftlReturn(1, PFTL_RECIPIENT, _pftlBytes(0x33), return_nonce);
        _assertTrue(replay_registry.consumed_return_nonce(return_nonce), "return nonce stored");

        WrappedVenueNAVCoin replacement_wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        PFTLUniswapHandoffController replacement = new PFTLUniswapHandoffController(
            IVenueMintableToken(address(replacement_wrapped)),
            IExactInputRouter(address(router)),
            verifier,
            _routeConfig(TRUST_CLASS_CONTROLLED)
        );
        replay_registry.setControllerAuthorization(address(replacement), true);
        replacement_wrapped.setController(address(replacement));
        replacement_wrapped.lockController();

        _expectMintOnlyRevertTo(replacement, packet);
        vm.prank(RECIPIENT);
        try replacement.burnForPftlReturn(1, PFTL_RECIPIENT, _pftlBytes(0x33), return_nonce) {
            revert("expected replayed return nonce revert");
        } catch {}
    }

    function testConsumeMintAndSwapMintsAndSwapsAtomically() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, keccak256(SWAP_DATA));
        bytes32 digest = controller.packetDigest(packet);

        (bytes32 consumed, uint256 amount_out) = controller.consumeMintAndSwap(packet, SWAP_DATA);

        _assertEq(consumed, digest, "digest");
        _assertEq(amount_out, 95, "amount out");
        _assertTrue(controller.consumed_packet(digest), "consumed");
        _assertEq(wrapped.balanceOf(address(controller)), 0, "controller wrapped");
        _assertEq(wrapped.balanceOf(address(router)), 10, "router received wrapped");
        _assertEq(usdc.balanceOf(RECIPIENT), 95, "recipient usdc");
        _assertEq(controller.total_minted_atoms(), 10, "total minted");
    }

    function testConsumeMintAndSwapThroughBoundAdapter() public {
        wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        controller = _controllerWithRouter(_boundAdapterRouter());
        wrapped.setController(address(controller));
        wrapped.lockController();
        adapter.setController(address(controller));
        adapter.lockController();

        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, keccak256(SWAP_DATA));
        bytes32 digest = controller.packetDigest(packet);

        (bytes32 consumed, uint256 amount_out) = controller.consumeMintAndSwap(packet, SWAP_DATA);

        _assertEq(consumed, digest, "digest");
        _assertEq(amount_out, 95, "amount out");
        _assertTrue(controller.consumed_packet(digest), "consumed");
        _assertEq(wrapped.balanceOf(address(controller)), 0, "controller wrapped");
        _assertEq(wrapped.balanceOf(address(adapter)), 0, "adapter wrapped");
        _assertEq(wrapped.balanceOf(address(router)), 10, "router received wrapped");
        _assertEq(usdc.balanceOf(RECIPIENT), 95, "recipient usdc");
    }

    function testAdapterPathMismatchDoesNotConsumePacket() public {
        wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        controller = _controllerWithRouter(_boundAdapterRouter());
        wrapped.setController(address(controller));
        wrapped.lockController();
        adapter.setController(address(controller));
        adapter.lockController();

        bytes memory bad_data = hex"05060708";
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, keccak256(bad_data));
        bytes32 digest = controller.packetDigest(packet);

        _expectSwapRevert(packet, bad_data);

        _assertTrue(!controller.consumed_packet(digest), "not consumed");
        _assertEq(controller.total_minted_atoms(), 0, "mint reverted");
        _assertEq(wrapped.balanceOf(address(controller)), 0, "controller wrapped reverted");
        _assertEq(wrapped.balanceOf(address(adapter)), 0, "adapter wrapped reverted");
        _assertEq(usdc.balanceOf(RECIPIENT), 0, "recipient unpaid");
    }

    function testSwapFailureDoesNotConsumePacket() public {
        router.setAmountOut(90);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, keccak256(SWAP_DATA));
        bytes32 digest = controller.packetDigest(packet);

        _expectSwapRevert(packet, SWAP_DATA);

        _assertTrue(!controller.consumed_packet(digest), "not consumed");
        _assertEq(controller.total_minted_atoms(), 0, "mint reverted");
        _assertEq(wrapped.balanceOf(address(controller)), 0, "controller wrapped reverted");
        _assertEq(usdc.balanceOf(RECIPIENT), 0, "recipient unpaid");
    }

    function testRouterReturnValueCannotOverstateDirectSwapSettlement() public {
        router.setReportedAndActualAmountOut(95, 94);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, keccak256(SWAP_DATA));
        bytes32 digest = controller.packetDigest(packet);

        _expectSwapRevert(packet, SWAP_DATA);

        _assertTrue(!controller.consumed_packet(digest), "not consumed");
        _assertEq(controller.total_minted_atoms(), 0, "mint reverted");
        _assertEq(wrapped.balanceOf(address(controller)), 0, "controller wrapped reverted");
        _assertEq(usdc.balanceOf(RECIPIENT), 0, "recipient unpaid");
    }

    function testRouterReturnValueCannotOverstateAdapterSettlement() public {
        wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        controller = _controllerWithRouter(_boundAdapterRouter());
        wrapped.setController(address(controller));
        wrapped.lockController();
        adapter.setController(address(controller));
        adapter.lockController();
        router.setReportedAndActualAmountOut(95, 94);

        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, keccak256(SWAP_DATA));
        bytes32 digest = controller.packetDigest(packet);

        _expectSwapRevert(packet, SWAP_DATA);

        _assertTrue(!controller.consumed_packet(digest), "not consumed");
        _assertEq(controller.total_minted_atoms(), 0, "mint reverted");
        _assertEq(wrapped.balanceOf(address(controller)), 0, "controller wrapped reverted");
        _assertEq(wrapped.balanceOf(address(adapter)), 0, "adapter wrapped reverted");
        _assertEq(usdc.balanceOf(RECIPIENT), 0, "recipient unpaid");
    }

    function testBadSwapPathHashFailsBeforeConsume() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(uint256(0x9999)));
        bytes32 digest = controller.packetDigest(packet);

        _expectSwapRevert(packet, SWAP_DATA);

        _assertTrue(!controller.consumed_packet(digest), "not consumed");
        _assertEq(controller.total_minted_atoms(), 0, "mint reverted");
    }

    function testConfigMismatchAndCapsFailClosed() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory wrong_config = _packet(100, 10, 1_500, bytes32(0));
        wrong_config.route_config_digest = _pftlBytes(0x99);
        _expectMintOnlyRevert(wrong_config);

        PFTLUniswapHandoffController.MintAndSwapPacket memory over_notional = _packet(1_001, 10, 1_500, bytes32(0));
        _expectMintOnlyRevert(over_notional);

        PFTLUniswapHandoffController.MintAndSwapPacket memory over_supply = _packet(100, 10_001, 1_500, bytes32(0));
        _expectMintOnlyRevert(over_supply);
    }

    function testPricingEpochAndReservePacketMismatchFailClosed() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory wrong_epoch = _packet(100, 10, 1_500, bytes32(0));
        wrong_epoch.pricing_nav_epoch = 8;
        bytes32 wrong_epoch_digest = controller.packetDigest(wrong_epoch);

        _expectMintOnlyRevert(wrong_epoch);

        _assertTrue(!controller.consumed_packet(wrong_epoch_digest), "wrong epoch not consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "wrong epoch unpaid");
        _assertEq(controller.total_minted_atoms(), 0, "wrong epoch did not mint");

        PFTLUniswapHandoffController.MintAndSwapPacket memory wrong_reserve = _packet(100, 10, 1_500, bytes32(0));
        wrong_reserve.pricing_reserve_packet_hash = _pftlBytes(0x88);
        bytes32 wrong_reserve_digest = controller.packetDigest(wrong_reserve);

        _expectMintOnlyRevert(wrong_reserve);

        _assertTrue(!controller.consumed_packet(wrong_reserve_digest), "wrong reserve not consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "wrong reserve unpaid");
        _assertEq(controller.total_minted_atoms(), 0, "wrong reserve did not mint");
    }

    function testWrongChainBridgeAndWrappedTokenFailClosed() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory wrong_chain = _packet(100, 10, 1_500, bytes32(0));
        wrong_chain.destination_chain_id = block.chainid + 1;
        bytes32 wrong_chain_digest = controller.packetDigest(wrong_chain);

        _expectMintOnlyRevert(wrong_chain);

        _assertTrue(!controller.consumed_packet(wrong_chain_digest), "wrong chain not consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "wrong chain unpaid");

        PFTLUniswapHandoffController.MintAndSwapPacket memory wrong_bridge = _packet(100, 10, 1_500, bytes32(0));
        wrong_bridge.destination_bridge = address(0xBAD);
        bytes32 wrong_bridge_digest = controller.packetDigest(wrong_bridge);

        _expectMintOnlyRevert(wrong_bridge);

        _assertTrue(!controller.consumed_packet(wrong_bridge_digest), "wrong bridge not consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "wrong bridge unpaid");

        PFTLUniswapHandoffController.MintAndSwapPacket memory wrong_token = _packet(100, 10, 1_500, bytes32(0));
        wrong_token.wrapped_navcoin_token = address(0xCAFE);
        bytes32 wrong_token_digest = controller.packetDigest(wrong_token);

        _expectMintOnlyRevert(wrong_token);

        _assertTrue(!controller.consumed_packet(wrong_token_digest), "wrong token not consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "wrong token unpaid");
    }

    function testModifiedRecipientFailsVerifierBinding() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        packet.ethereum_recipient = address(0xCAFE);
        bytes32 digest = controller.packetDigest(packet);

        _expectMintOnlyRevert(packet);

        _assertTrue(!controller.consumed_packet(digest), "modified recipient not consumed");
        _assertEq(wrapped.balanceOf(address(0xCAFE)), 0, "modified recipient unpaid");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "original recipient unpaid");
    }

    function testReentrancyAttemptIsRejectedAndOuterSwapSettlesOnce() public {
        wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        HandoffReentrantRouter reentrant = new HandoffReentrantRouter(POOL_ID);
        controller = _controllerWithRouter(IExactInputRouter(address(reentrant)));
        wrapped.setController(address(controller));
        wrapped.lockController();
        controller.setExecutorApproval(address(reentrant), true);

        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, keccak256(SWAP_DATA));
        reentrant.setAttack(controller, packet, SWAP_DATA);
        bytes32 digest = controller.packetDigest(packet);

        (bytes32 consumed, uint256 amount_out) = controller.consumeMintAndSwap(packet, SWAP_DATA);

        _assertEq(consumed, digest, "digest");
        _assertEq(amount_out, 95, "amount out");
        _assertTrue(reentrant.reentry_attempted(), "reentry attempted");
        _assertTrue(reentrant.reentry_rejected(), "reentry rejected");
        _assertTrue(controller.consumed_packet(digest), "outer packet consumed");
        _assertEq(controller.total_minted_atoms(), 10, "one outer mint");
        _assertEq(wrapped.balanceOf(address(controller)), 0, "controller wrapped settled");
        _assertEq(wrapped.balanceOf(address(reentrant)), 10, "router received wrapped once");
        _assertEq(usdc.balanceOf(RECIPIENT), 95, "recipient paid once");
    }

    function testUnacceptedReceiptFailsClosed() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        packet.source_receipt_hash = _pftlBytes(0x88);
        bytes32 digest = controller.packetDigest(packet);

        _expectMintOnlyRevert(packet);

        _assertTrue(!controller.consumed_packet(digest), "not consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 0, "recipient unpaid");
    }

    function testOptimisticReceiptFinalizesAndAllowsMintOnly() public {
        OptimisticPFTLReceiptVerifier optimistic = new OptimisticPFTLReceiptVerifier(
            address(this),
            OPTIMISTIC_POSTER_BOND,
            OPTIMISTIC_CHALLENGER_BOND,
            OPTIMISTIC_CHALLENGE_WINDOW,
            OPTIMISTIC_CHALLENGE_RESOLUTION_WINDOW
        );
        _installOptimisticController(optimistic);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);

        vm.deal(OPTIMISTIC_POSTER, OPTIMISTIC_POSTER_BOND);
        vm.prank(OPTIMISTIC_POSTER);
        bytes32 claim_id = optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND}(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest
        );

        _assertTrue(
            !optimistic.isReceiptAccepted(
                packet.source_receipt_root,
                packet.source_receipt_hash,
                packet.route_config_digest,
                TRUST_CLASS_OPTIMISTIC,
                digest
            ),
            "challenge window not accepted"
        );
        _expectMintOnlyRevert(packet);

        vm.warp(1_101);
        optimistic.finalizeReceiptClaim(claim_id);

        _assertTrue(
            optimistic.isReceiptAccepted(
                packet.source_receipt_root,
                packet.source_receipt_hash,
                packet.route_config_digest,
                TRUST_CLASS_OPTIMISTIC,
                digest
            ),
            "claim accepted"
        );
        bytes32 consumed = controller.consumeMintOnly(packet);

        _assertEq(consumed, digest, "optimistic digest");
        _assertTrue(controller.consumed_packet(digest), "optimistic consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "optimistic mint");
        _assertEq(optimistic.bond_credit_wei(OPTIMISTIC_POSTER), OPTIMISTIC_POSTER_BOND, "poster bond credit");
        vm.prank(OPTIMISTIC_POSTER);
        optimistic.withdrawBondCredit();
        _assertEq(OPTIMISTIC_POSTER.balance, OPTIMISTIC_POSTER_BOND, "poster bond withdrawn");
    }

    function testOptimisticSourceReceiptReuseCannotPostSecondAcceptedClaim() public {
        OptimisticPFTLReceiptVerifier optimistic = new OptimisticPFTLReceiptVerifier(
            address(this),
            OPTIMISTIC_POSTER_BOND,
            OPTIMISTIC_CHALLENGER_BOND,
            OPTIMISTIC_CHALLENGE_WINDOW,
            OPTIMISTIC_CHALLENGE_RESOLUTION_WINDOW
        );
        _installOptimisticController(optimistic);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);
        bytes32 source_receipt_commitment = _sourceReceiptReplayCommitment(packet);

        vm.deal(OPTIMISTIC_POSTER, OPTIMISTIC_POSTER_BOND);
        vm.prank(OPTIMISTIC_POSTER);
        bytes32 claim_id = optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND}(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest
        );
        vm.warp(1_101);
        optimistic.finalizeReceiptClaim(claim_id);
        controller.consumeMintOnly(packet);

        _assertEq(optimistic.source_receipt_claim_id(source_receipt_commitment), claim_id, "source claim pinned");

        PFTLUniswapHandoffController.MintAndSwapPacket memory mutated =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        mutated.source_packet_hash = _pftlBytes(0x99);
        mutated.nonce = bytes32(uint256(0x9998));
        bytes32 mutated_digest = controller.packetDigest(mutated);

        vm.deal(OPTIMISTIC_POSTER, OPTIMISTIC_POSTER_BOND);
        vm.prank(OPTIMISTIC_POSTER);
        try optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND}(
            mutated.source_receipt_root, mutated.source_receipt_hash, mutated.route_config_digest, mutated_digest
        ) {
            revert("expected duplicate source receipt claim revert");
        } catch {}

        _assertTrue(
            !optimistic.isReceiptAccepted(
                mutated.source_receipt_root,
                mutated.source_receipt_hash,
                mutated.route_config_digest,
                TRUST_CLASS_OPTIMISTIC,
                mutated_digest
            ),
            "mutated receipt not accepted"
        );
        _expectMintOnlyRevert(mutated);
        _assertTrue(!controller.consumed_packet(mutated_digest), "mutated packet not consumed");
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "recipient unchanged");
    }

    function testOptimisticValidChallengeRejectsBeforeSettlementAndPaysBonds() public {
        OptimisticPFTLReceiptVerifier optimistic = new OptimisticPFTLReceiptVerifier(
            address(this),
            OPTIMISTIC_POSTER_BOND,
            OPTIMISTIC_CHALLENGER_BOND,
            OPTIMISTIC_CHALLENGE_WINDOW,
            OPTIMISTIC_CHALLENGE_RESOLUTION_WINDOW
        );
        _installOptimisticController(optimistic);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);

        vm.deal(OPTIMISTIC_POSTER, OPTIMISTIC_POSTER_BOND);
        vm.prank(OPTIMISTIC_POSTER);
        bytes32 claim_id = optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND}(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest
        );

        vm.deal(OPTIMISTIC_CHALLENGER, OPTIMISTIC_CHALLENGER_BOND);
        vm.prank(OPTIMISTIC_CHALLENGER);
        optimistic.challengeReceiptClaim{value: OPTIMISTIC_CHALLENGER_BOND}(
            claim_id,
            OptimisticPFTLReceiptVerifier.ChallengeFault.InvalidReceiptHash,
            OPTIMISTIC_CHALLENGE_EVIDENCE_HASH
        );

        _assertTrue(
            !optimistic.isReceiptAccepted(
                packet.source_receipt_root,
                packet.source_receipt_hash,
                packet.route_config_digest,
                TRUST_CLASS_OPTIMISTIC,
                digest
            ),
            "challenged not accepted"
        );
        _expectMintOnlyRevert(packet);

        optimistic.resolveReceiptChallenge(claim_id, true);
        _assertEq(
            optimistic.bond_credit_wei(OPTIMISTIC_CHALLENGER),
            OPTIMISTIC_POSTER_BOND + OPTIMISTIC_CHALLENGER_BOND,
            "valid challenge credit"
        );
        try optimistic.finalizeReceiptClaim(claim_id) {
            revert("expected rejected finalize revert");
        } catch {}
        _expectMintOnlyRevert(packet);
        _assertTrue(!controller.consumed_packet(digest), "challenged not consumed");
    }

    function testOptimisticResolverGovernanceRotatesResolver() public {
        address new_resolver = address(0xA11CE);
        address not_owner = address(0xB0B);
        OptimisticPFTLReceiptVerifier optimistic = new OptimisticPFTLReceiptVerifier(
            address(this),
            OPTIMISTIC_POSTER_BOND,
            OPTIMISTIC_CHALLENGER_BOND,
            OPTIMISTIC_CHALLENGE_WINDOW,
            OPTIMISTIC_CHALLENGE_RESOLUTION_WINDOW
        );
        _installOptimisticController(optimistic);
        _assertTrue(optimistic.owner() == address(this), "initial owner");
        _assertTrue(optimistic.challenge_resolver() == address(this), "initial resolver");

        vm.prank(not_owner);
        try optimistic.setChallengeResolver(new_resolver) {
            revert("expected non-owner resolver rotation revert");
        } catch {}
        try optimistic.setChallengeResolver(address(0)) {
            revert("expected zero resolver revert");
        } catch {}

        optimistic.setChallengeResolver(new_resolver);
        _assertTrue(optimistic.challenge_resolver() == new_resolver, "rotated resolver");

        PFTLUniswapHandoffController.MintAndSwapPacket memory packet =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);

        vm.deal(OPTIMISTIC_POSTER, OPTIMISTIC_POSTER_BOND);
        vm.prank(OPTIMISTIC_POSTER);
        bytes32 claim_id = optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND}(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest
        );

        vm.deal(OPTIMISTIC_CHALLENGER, OPTIMISTIC_CHALLENGER_BOND);
        vm.prank(OPTIMISTIC_CHALLENGER);
        optimistic.challengeReceiptClaim{value: OPTIMISTIC_CHALLENGER_BOND}(
            claim_id,
            OptimisticPFTLReceiptVerifier.ChallengeFault.InvalidReceiptHash,
            OPTIMISTIC_CHALLENGE_EVIDENCE_HASH
        );

        try optimistic.resolveReceiptChallenge(claim_id, true) {
            revert("expected old resolver revert");
        } catch {}
        vm.prank(new_resolver);
        optimistic.resolveReceiptChallenge(claim_id, true);
        _expectMintOnlyRevert(packet);

        address next_owner = address(0xA11C2);
        optimistic.transferOwnership(next_owner);
        _assertTrue(optimistic.owner() == next_owner, "owner transferred");
        try optimistic.setChallengeResolver(address(this)) {
            revert("expected previous owner cannot rotate resolver");
        } catch {}
        vm.prank(next_owner);
        optimistic.setChallengeResolver(address(this));
        _assertTrue(optimistic.challenge_resolver() == address(this), "new owner rotated resolver");
    }

    function testOptimisticUnresolvedChallengeFailsClosedAfterResolutionDeadline() public {
        OptimisticPFTLReceiptVerifier optimistic = new OptimisticPFTLReceiptVerifier(
            address(this),
            OPTIMISTIC_POSTER_BOND,
            OPTIMISTIC_CHALLENGER_BOND,
            OPTIMISTIC_CHALLENGE_WINDOW,
            OPTIMISTIC_CHALLENGE_RESOLUTION_WINDOW
        );
        _installOptimisticController(optimistic);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);

        vm.deal(OPTIMISTIC_POSTER, OPTIMISTIC_POSTER_BOND);
        vm.prank(OPTIMISTIC_POSTER);
        bytes32 claim_id = optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND}(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest
        );

        vm.deal(OPTIMISTIC_CHALLENGER, OPTIMISTIC_CHALLENGER_BOND);
        vm.prank(OPTIMISTIC_CHALLENGER);
        optimistic.challengeReceiptClaim{value: OPTIMISTIC_CHALLENGER_BOND}(
            claim_id,
            OptimisticPFTLReceiptVerifier.ChallengeFault.InvalidReceiptHash,
            OPTIMISTIC_CHALLENGE_EVIDENCE_HASH
        );
        _expectMintOnlyRevert(packet);

        vm.warp(1_101);
        optimistic.finalizeReceiptClaim(claim_id);
        _assertTrue(
            !optimistic.isReceiptAccepted(
                packet.source_receipt_root,
                packet.source_receipt_hash,
                packet.route_config_digest,
                TRUST_CLASS_OPTIMISTIC,
                digest
            ),
            "unresolved challenge not accepted"
        );
        _expectMintOnlyRevert(packet);
        _assertTrue(!controller.consumed_packet(digest), "timed-out challenge not consumed");
        _assertEq(optimistic.bond_credit_wei(OPTIMISTIC_POSTER), OPTIMISTIC_POSTER_BOND, "poster bond refunded");
        _assertEq(
            optimistic.bond_credit_wei(OPTIMISTIC_CHALLENGER), OPTIMISTIC_CHALLENGER_BOND, "challenger bond refunded"
        );
    }

    function testOptimisticUnderbondedAndLateChallengeRejected() public {
        OptimisticPFTLReceiptVerifier optimistic = new OptimisticPFTLReceiptVerifier(
            address(this),
            OPTIMISTIC_POSTER_BOND,
            OPTIMISTIC_CHALLENGER_BOND,
            OPTIMISTIC_CHALLENGE_WINDOW,
            OPTIMISTIC_CHALLENGE_RESOLUTION_WINDOW
        );
        _installOptimisticController(optimistic);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        bytes32 digest = controller.packetDigest(packet);

        try optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND - 1}(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest
        ) {
            revert("expected underbonded post revert");
        } catch {}

        vm.deal(OPTIMISTIC_POSTER, OPTIMISTIC_POSTER_BOND);
        vm.prank(OPTIMISTIC_POSTER);
        bytes32 claim_id = optimistic.postReceiptClaim{value: OPTIMISTIC_POSTER_BOND}(
            packet.source_receipt_root, packet.source_receipt_hash, packet.route_config_digest, digest
        );

        vm.warp(1_101);
        vm.deal(OPTIMISTIC_CHALLENGER, OPTIMISTIC_CHALLENGER_BOND);
        vm.prank(OPTIMISTIC_CHALLENGER);
        try optimistic.challengeReceiptClaim{value: OPTIMISTIC_CHALLENGER_BOND}(
            claim_id,
            OptimisticPFTLReceiptVerifier.ChallengeFault.InvalidReceiptHash,
            OPTIMISTIC_CHALLENGE_EVIDENCE_HASH
        ) {
            revert("expected late challenge revert");
        } catch {}

        optimistic.finalizeReceiptClaim(claim_id);
        bytes32 consumed = controller.consumeMintOnly(packet);
        _assertEq(consumed, digest, "late challenge did not grief");
    }

    function testVerifierTrustClassMismatchFailsConstructor() public {
        try new ControlledPFTLReceiptVerifier(address(this), keccak256("OPTIMISTIC")) returns (
            ControlledPFTLReceiptVerifier
        ) {
            revert("controlled owner-toggle verifier cannot claim optimistic verification");
        } catch {}

        try new ControlledPFTLReceiptVerifier(address(this), keccak256("TRUSTLESS_FINALITY")) returns (
            ControlledPFTLReceiptVerifier
        ) {
            revert("controlled owner-toggle verifier cannot claim trustless finality");
        } catch {}
    }

    function testRouterPoolMismatchFailsConstructor() public {
        HandoffMockRouter wrong_pool_router = new HandoffMockRouter(bytes32(uint256(0x9999)));
        try new PFTLUniswapHandoffController(
            IVenueMintableToken(address(wrapped)),
            IExactInputRouter(address(wrong_pool_router)),
            verifier,
            _routeConfig(TRUST_CLASS_CONTROLLED)
        ) returns (
            PFTLUniswapHandoffController
        ) {
            revert("expected router pool mismatch");
        } catch {}
    }

    function testDisabledRouteAndUnauthorizedExecutorFailClosed() public {
        verifier = new ControlledPFTLReceiptVerifier(address(this), TRUST_CLASS_DISABLED);
        PFTLUniswapHandoffController disabled = _controller(TRUST_CLASS_DISABLED);
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 1_500, bytes32(0));
        _expectMintOnlyRevertTo(disabled, packet);

        vm.prank(address(0xBAD));
        _expectMintOnlyRevert(packet);
    }

    function testDeadlineExpiredFailsClosed() public {
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet = _packet(100, 10, 999, bytes32(0));
        _expectMintOnlyRevert(packet);
    }

    function testThresholdPftlFinalityCertificateDrivesControllerMint() public {
        uint256[] memory keys = _sortedBridgeKeys();
        address[] memory signers = new address[](keys.length);
        for (uint256 i = 0; i < keys.length; i++) {
            signers[i] = vm.addr(keys[i]);
        }
        ThresholdPFTLReceiptVerifier threshold_verifier =
            new ThresholdPFTLReceiptVerifier(keccak256("postfiat-devnet"), keccak256("genesis"), 1, 7, signers, 3);
        wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        controller = new PFTLUniswapHandoffController(
            IVenueMintableToken(address(wrapped)),
            IExactInputRouter(address(router)),
            threshold_verifier,
            _routeConfig(TRUST_CLASS_BFT_CHECKPOINT)
        );
        replay_registry.setControllerAuthorization(address(controller), true);
        wrapped.setController(address(controller));
        wrapped.lockController();

        PFTLUniswapHandoffController.MintAndSwapPacket memory packet =
            _packetWithoutReceiptAcceptance(100, 10, 1_500, bytes32(0));
        bytes32 packet_digest = controller.packetDigest(packet);
        bytes32 receipt_code = threshold_verifier.ACCEPTED_RECEIPT_CODE();
        bytes32 certificate_digest = threshold_verifier.certificateDigest(
            packet.source_receipt_root,
            packet.source_receipt_hash,
            packet.route_config_digest,
            packet_digest,
            1_212,
            receipt_code
        );
        bytes[] memory signatures = new bytes[](3);
        for (uint256 i = 0; i < signatures.length; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], certificate_digest);
            signatures[i] = abi.encodePacked(r, s, v);
        }
        threshold_verifier.submitReceiptCertificate(
            packet.source_receipt_root,
            packet.source_receipt_hash,
            packet.route_config_digest,
            packet_digest,
            1_212,
            receipt_code,
            signatures
        );

        bytes32 consumed = controller.consumeMintOnly(packet);
        _assertEq(consumed, packet_digest, "threshold packet digest");
        _assertEq(wrapped.balanceOf(RECIPIENT), 10, "threshold-certified mint");
    }

    function _sortedBridgeKeys() private returns (uint256[] memory keys) {
        keys = new uint256[](4);
        keys[0] = 0xB101;
        keys[1] = 0xB102;
        keys[2] = 0xB103;
        keys[3] = 0xB104;
        for (uint256 i = 1; i < keys.length; i++) {
            uint256 key = keys[i];
            address signer = vm.addr(key);
            uint256 cursor = i;
            while (cursor > 0 && vm.addr(keys[cursor - 1]) > signer) {
                keys[cursor] = keys[cursor - 1];
                cursor -= 1;
            }
            keys[cursor] = key;
        }
    }

    function _controller(bytes32 trust_class) private returns (PFTLUniswapHandoffController) {
        return _controllerWithRouter(IExactInputRouter(address(router)), trust_class);
    }

    function _controllerWithRouter(IExactInputRouter router_) private returns (PFTLUniswapHandoffController) {
        return _controllerWithRouter(router_, TRUST_CLASS_CONTROLLED);
    }

    function _controllerWithRouter(IExactInputRouter router_, bytes32 trust_class)
        private
        returns (PFTLUniswapHandoffController)
    {
        PFTLUniswapHandoffController next_controller = new PFTLUniswapHandoffController(
            IVenueMintableToken(address(wrapped)), router_, verifier, _routeConfig(trust_class)
        );
        replay_registry.setControllerAuthorization(address(next_controller), true);
        return next_controller;
    }

    function _installOptimisticController(OptimisticPFTLReceiptVerifier optimistic) private {
        wrapped = new WrappedVenueNAVCoin("Wrapped A666", "wA666", 6, address(this));
        controller = new PFTLUniswapHandoffController(
            IVenueMintableToken(address(wrapped)),
            IExactInputRouter(address(router)),
            optimistic,
            _routeConfig(TRUST_CLASS_OPTIMISTIC)
        );
        replay_registry.setControllerAuthorization(address(controller), true);
        wrapped.setController(address(controller));
        wrapped.lockController();
    }

    function _routeConfig(bytes32 trust_class)
        private
        view
        returns (PFTLUniswapHandoffController.RouteConfig memory config)
    {
        config.initial_owner = address(this);
        config.destination_chain_id = block.chainid;
        config.route_config_digest = _pftlBytes(0x11);
        config.route_trust_class = trust_class;
        config.settlement_asset_id = _pftlBytes(0x22);
        config.native_nav_asset_id = _pftlBytes(0x33);
        config.pricing_reserve_packet_hash = _pftlBytes(0x55);
        config.pricing_nav_epoch = 7;
        config.uniswap_pool_id = POOL_ID;
        config.route_supply_cap_atoms = 10_000;
        config.packet_notional_cap_atoms = 1_000;
        config.replay_registry = address(replay_registry);
    }

    function _boundAdapterRouter() private returns (IExactInputRouter) {
        adapter = new UniswapSettlementAdapter(
            IExactInputRouter(address(router)),
            address(wrapped),
            address(usdc),
            POOL_ID,
            keccak256(SWAP_DATA),
            address(this)
        );
        return IExactInputRouter(address(adapter));
    }

    function _packet(uint256 settlement_amount, uint256 mint_amount, uint64 deadline, bytes32 swap_path_hash)
        private
        returns (PFTLUniswapHandoffController.MintAndSwapPacket memory packet)
    {
        packet = _packetWithoutReceiptAcceptance(settlement_amount, mint_amount, deadline, swap_path_hash);
        verifier.setReceiptAcceptance(
            packet.source_receipt_root,
            packet.source_receipt_hash,
            packet.route_config_digest,
            controller.packetDigest(packet),
            true
        );
    }

    function _packetWithoutReceiptAcceptance(
        uint256 settlement_amount,
        uint256 mint_amount,
        uint64 deadline,
        bytes32 swap_path_hash
    ) private view returns (PFTLUniswapHandoffController.MintAndSwapPacket memory packet) {
        packet.route_config_digest = _pftlBytes(0x11);
        packet.source_packet_hash = _pftlBytes(0x12);
        packet.source_receipt_hash = _pftlBytes(0x13);
        packet.source_receipt_root = _pftlBytes(0x14);
        packet.destination_chain_id = block.chainid;
        packet.destination_bridge = address(controller);
        packet.wrapped_navcoin_token = address(wrapped);
        packet.source_wallet_hash = WALLET_HASH;
        packet.settlement_asset_id = _pftlBytes(0x22);
        packet.native_nav_asset_id = _pftlBytes(0x33);
        packet.pricing_reserve_packet_hash = _pftlBytes(0x55);
        packet.uniswap_pool_id = POOL_ID;
        packet.swap_path_hash = swap_path_hash;
        packet.ethereum_recipient = RECIPIENT;
        packet.token_out = address(usdc);
        packet.settlement_amount_atoms = settlement_amount;
        packet.mint_amount_atoms = mint_amount;
        packet.minimum_output_atoms = 95;
        packet.pricing_nav_epoch = 7;
        packet.deadline = deadline;
        packet.nonce = NONCE;
    }

    function _uniqueAcceptedPacket(
        uint8 source_packet_value,
        uint8 source_receipt_value,
        bytes32 nonce,
        uint256 settlement_amount,
        uint256 mint_amount,
        uint64 deadline,
        bytes32 swap_path_hash
    ) private returns (PFTLUniswapHandoffController.MintAndSwapPacket memory packet) {
        packet = _packetWithoutReceiptAcceptance(settlement_amount, mint_amount, deadline, swap_path_hash);
        packet.source_packet_hash = _pftlBytes(source_packet_value);
        packet.source_receipt_hash = _pftlBytes(source_receipt_value);
        packet.nonce = nonce;
        verifier.setReceiptAcceptance(
            packet.source_receipt_root,
            packet.source_receipt_hash,
            packet.route_config_digest,
            controller.packetDigest(packet),
            true
        );
    }

    function _pftlBytes(uint8 value) private pure returns (bytes memory result) {
        result = new bytes(48);
        for (uint256 i = 0; i < result.length; i++) {
            result[i] = bytes1(value);
        }
    }

    function _sourceReceiptReplayCommitment(PFTLUniswapHandoffController.MintAndSwapPacket memory packet)
        private
        pure
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.source_receipt.v1", packet.source_receipt_root, packet.source_receipt_hash
            )
        );
    }

    function _expectMintOnlyRevert(PFTLUniswapHandoffController.MintAndSwapPacket memory packet) private {
        _expectMintOnlyRevertTo(controller, packet);
    }

    function _expectMintOnlyRevertTo(
        PFTLUniswapHandoffController target,
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet
    ) private {
        try target.consumeMintOnly(packet) returns (bytes32) {
            revert("expected consumeMintOnly revert");
        } catch {}
    }

    function _expectSwapRevert(PFTLUniswapHandoffController.MintAndSwapPacket memory packet, bytes memory data)
        private
    {
        try controller.consumeMintAndSwap(packet, data) returns (bytes32, uint256) {
            revert("expected consumeMintAndSwap revert");
        } catch {}
    }

    function _expectReturnBurnRevert(
        uint256 amount,
        string memory pftl_recipient,
        bytes memory native_asset_id,
        bytes32 nonce
    ) private {
        try controller.burnForPftlReturn(amount, pftl_recipient, native_asset_id, nonce) returns (bytes32) {
            revert("expected burnForPftlReturn revert");
        } catch {}
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) {
            revert(message);
        }
    }

    function _assertEq(bytes32 actual, bytes32 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}

contract HandoffMockToken {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    uint256 public totalSupply;

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 current_allowance = allowance[from][msg.sender];
        if (current_allowance < amount) {
            return false;
        }
        allowance[from][msg.sender] = current_allowance - amount;
        uint256 balance = balanceOf[from];
        if (balance < amount) {
            return false;
        }
        balanceOf[from] = balance - amount;
        balanceOf[to] += amount;
        return true;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }
}

contract HandoffMockRouter is IExactInputRouter {
    uint256 public amount_out;
    uint256 public actual_amount_out;
    bytes32 public immutable uniswap_pool_id;

    constructor(bytes32 uniswap_pool_id_) {
        uniswap_pool_id = uniswap_pool_id_;
    }

    function setAmountOut(uint256 amount_out_) external {
        amount_out = amount_out_;
        actual_amount_out = amount_out_;
    }

    function setReportedAndActualAmountOut(uint256 reported_amount_out, uint256 actual_amount_out_) external {
        amount_out = reported_amount_out;
        actual_amount_out = actual_amount_out_;
    }

    function exactInput(
        address token_in,
        address token_out,
        uint256 amount_in,
        uint256 minimum_output,
        address recipient,
        uint256,
        bytes calldata
    ) external returns (uint256) {
        bool pulled = IVenueMintableToken(token_in).transferFrom(msg.sender, address(this), amount_in);
        if (!pulled) {
            revert("router pull failed");
        }
        if (amount_out < minimum_output) {
            revert("router output below minimum");
        }
        HandoffMockToken(token_out).mint(recipient, actual_amount_out);
        return amount_out;
    }
}

contract HandoffReentrantRouter is IExactInputRouter {
    PFTLUniswapHandoffController private target;
    PFTLUniswapHandoffController.MintAndSwapPacket private packet;
    bytes private swap_data;
    bool public reentry_attempted;
    bool public reentry_rejected;
    bytes32 public immutable uniswap_pool_id;

    constructor(bytes32 uniswap_pool_id_) {
        uniswap_pool_id = uniswap_pool_id_;
    }

    function setAttack(
        PFTLUniswapHandoffController target_,
        PFTLUniswapHandoffController.MintAndSwapPacket memory packet_,
        bytes memory swap_data_
    ) external {
        target = target_;
        packet = packet_;
        swap_data = swap_data_;
    }

    function exactInput(
        address token_in,
        address token_out,
        uint256 amount_in,
        uint256 minimum_output,
        address recipient,
        uint256,
        bytes calldata
    ) external returns (uint256) {
        reentry_attempted = true;
        try target.consumeMintAndSwap(packet, swap_data) returns (bytes32, uint256) {
            revert("reentry unexpectedly succeeded");
        } catch {
            reentry_rejected = true;
        }
        bool pulled = IVenueMintableToken(token_in).transferFrom(msg.sender, address(this), amount_in);
        if (!pulled) {
            revert("router pull failed");
        }
        HandoffMockToken(token_out).mint(recipient, minimum_output);
        return minimum_output;
    }
}
