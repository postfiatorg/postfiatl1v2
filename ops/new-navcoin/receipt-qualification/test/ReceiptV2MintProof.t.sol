// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;
import {ReceiptV2ProofFixture} from "./ReceiptV2RealProof.t.sol";
import {PFTLReceiptFinalityVerifierV2} from "../src/PFTLReceiptFinalityVerifierV2.sol";
import {WrappedVenueNAVCoin} from "../src/PFTLUniswapHandoffController.sol";
import {PFTLUniswapPrimaryMarketV2, IA666WrappedToken, IPFTLReceiptFinalityVerifierV1}
    from "../src/PFTLUniswapPrimaryMarketV2.sol";
import {PFTLUniswapPrimaryMarketV3} from "../src/PFTLUniswapPrimaryMarketV3.sol";
import {PFTLReturnRouteV3} from "../src/PFTLReturnRouteV3.sol";
import {RealReceiptPacketFixture} from "../src/PacketFixture.sol";

/// Local constructor execution at fixture-bound addresses. No deployed state is
/// copied and no verifier acceptance/replay/supply storage is injected.
contract ReceiptV2MintFixture is ReceiptV2ProofFixture {
    WrappedVenueNAVCoin wrapped;
    PFTLUniswapPrimaryMarketV3 market;
    PFTLUniswapPrimaryMarketV2.MintPacket packet;

    function deployAt(address target, bytes memory creation) internal {
        vm.etch(target, creation);
        (bool ok, bytes memory runtime) = target.call("");
        if (!ok) { assembly { revert(add(runtime, 32), mload(runtime)) } }
        require(runtime.length != 0, "constructor returned empty runtime");
        vm.etch(target, runtime);
    }

    function deployMarket() internal {
        PFTLUniswapPrimaryMarketV2.Config memory c = PFTLUniswapPrimaryMarketV2.Config({
            destinationChainId: packet.destinationChainId,
            settlementAssetId: packet.settlementAssetId, nativeNavAssetId: packet.nativeNavAssetId,
            uniswapPoolId: bytes32(uint256(1)), routeSupplyCapAtoms: 2_000_000e6,
            packetNotionalCapAtoms: 250_000e6, governance: address(this)
        });
        deployAt(packet.destinationController, abi.encodePacked(
            type(PFTLUniswapPrimaryMarketV3).creationCode,
            abi.encode(IA666WrappedToken(address(wrapped)),
                IPFTLReceiptFinalityVerifierV1(address(verifier)), c, RealReceiptPacketFixture.domain())
        ));
        market = PFTLUniswapPrimaryMarketV3(packet.destinationController);
        require(market.mintPaused(), "constructor must start paused");
        require(market.returnRouteBinding() != bytes32(0), "missing V3 return domain");
    }

    function setUp() public override {
        super.setUp();
        require(isReceipt, "mint tests require a receipt");
        packet = RealReceiptPacketFixture.packet();
        require(packet.wrappedToken == receiptValues.wrappedToken
            && packet.destinationController == receiptValues.controller, "wrong witness addresses");
        deployAt(packet.wrappedToken, abi.encodePacked(type(WrappedVenueNAVCoin).creationCode,
            abi.encode("Local receipt token", "rNAV", uint8(6), address(this))));
        wrapped = WrappedVenueNAVCoin(packet.wrappedToken);
        // Recreate the verifier with the actual initialized token runtime hash.
        verifier = new PFTLReceiptFinalityVerifierV2(config());
        deployMarket();
        wrapped.setController(address(market));
        wrapped.lockController();
        require(wrapped.controller_locked() && wrapped.owner() == address(this), "token constructor/roles");
        require(market.packetDigest(packet) == receiptValues.packetDigest, "witness/proof packet mismatch");
        require(keccak256(packet.nativeNavAssetId) == receiptValues.nativeNavAssetIdCommitment
            && keccak256(packet.settlementAssetId) == receiptValues.settlementAssetIdCommitment,
            "witness/proof asset mismatch");
        PFTLReturnRouteV3.ReturnRouteDomain memory domain = RealReceiptPacketFixture.domain();
        require(keccak256(bytes(domain.chain_id)) == receiptValues.pftlChainIdHash
            && keccak256(domain.genesis_hash) == receiptValues.pftlGenesisHashCommitment
            && domain.protocol_version == receiptValues.pftlProtocolVersion
            && keccak256(bytes(domain.route_id)) == receiptValues.routeIdCommitment,
            "witness/proof return domain mismatch");
        require(keccak256(packet.sourceReceiptHash) == receiptValues.sourceReceiptHashCommitment
            && keccak256(packet.sourceReceiptRoot) == receiptValues.sourceReceiptRootCommitment,
            "witness/proof receipt mismatch");
        vm.warp(packet.deadline - 1);
        noMint();
    }

    function noMint() internal view {
        require(wrapped.totalSupply() == 0 && wrapped.balanceOf(packet.ethereumRecipient) == 0,
            "unbacked tokens minted");
        require(market.totalMintedAtoms() == 0 && !market.consumedPacket(receiptValues.packetDigest),
            "controller state changed");
    }

    function proveAndMint() internal {
        sp1.verifyProof(key, publicValues, proof);
        accept(publicValues, proof);
        market.setMintPaused(false);
        market.consumeMintOnly(packet);
        require(wrapped.totalSupply() == packet.mintAmountAtoms
            && wrapped.balanceOf(packet.ethereumRecipient) == packet.mintAmountAtoms, "wrong supply");
        require(market.outstandingMintedAtoms() == packet.mintAmountAtoms, "wrong outstanding supply");
    }

}
contract ReceiptV2MintProofTest is ReceiptV2MintFixture {
    function testGenuineReceiptMintsOnceThroughProductionContracts() public {
        sp1.verifyProof(key, publicValues, proof);
        accept(publicValues, proof);
        vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.MintingPaused.selector));
        market.consumeMintOnly(packet);
        noMint();
        market.setMintPaused(false);
        market.consumeMintOnly(packet);
        require(wrapped.totalSupply() == packet.mintAmountAtoms
            && wrapped.balanceOf(packet.ethereumRecipient) == packet.mintAmountAtoms, "wrong mint");
        vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.PacketReplay.selector,
            receiptValues.packetDigest));
        market.consumeMintOnly(packet);
        require(wrapped.totalSupply() == packet.mintAmountAtoms, "replay minted twice");
    }

    function testUntrustedCommitteeCannotMintWithValidSP1Proof() public {
        sp1.verifyProof(key, publicValues, proof);
        PFTLReceiptFinalityVerifierV2.Config memory c = config();
        c.initialCommitteeRootCommitment ^= bytes32(uint256(1));
        verifier = new PFTLReceiptFinalityVerifierV2(c);
        deployMarket();
        vm.expectRevert(); accept(publicValues, proof);
        unchanged();
        market.setMintPaused(false);
        vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.ReceiptNotAccepted.selector,
            receiptValues.packetDigest));
        market.consumeMintOnly(packet);
        noMint();
    }

    function testCorruptedProofCannotMintThroughProductionController() public {
        proof[proof.length - 1] = bytes1(uint8(proof[proof.length - 1]) ^ 1);
        vm.expectRevert(); accept(publicValues, proof);
        unchanged();
        market.setMintPaused(false);
        vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.ReceiptNotAccepted.selector,
            receiptValues.packetDigest));
        market.consumeMintOnly(packet);
        noMint();
    }

    function testGenuineMintBurnsOnceWithBoundV3Return() public {
        proveAndMint();
        bytes32 nonce = bytes32(uint256(123));
        string memory recipient = "local-return-recipient";
        bytes32 base = keccak256(abi.encode("postfiat.pftl_uniswap.return_burn.v1", block.chainid,
            address(market), address(wrapped), packet.nativeNavAssetId, packet.ethereumRecipient,
            recipient, packet.mintAmountAtoms, nonce, block.number));
        bytes32 expected = keccak256(abi.encode("postfiat.pftl_uniswap.return_burn.v3", base,
            market.returnRouteBinding()));
        vm.prank(packet.ethereumRecipient);
        bytes32 actual = market.burnForPftlReturn(packet.mintAmountAtoms, recipient,
            packet.nativeNavAssetId, nonce);
        require(actual == expected, "return domain mismatch");
        require(wrapped.totalSupply() == 0 && wrapped.balanceOf(packet.ethereumRecipient) == 0
            && market.outstandingMintedAtoms() == 0, "burn accounting mismatch");
        vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.ReturnNonceReplay.selector, nonce));
        vm.prank(packet.ethereumRecipient);
        market.burnForPftlReturn(packet.mintAmountAtoms, recipient, packet.nativeNavAssetId, nonce);
    }
}
