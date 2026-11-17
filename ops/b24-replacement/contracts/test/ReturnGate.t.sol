// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;
import {PFTLUniswapPrimaryMarketV2,IA666WrappedToken,IPFTLReceiptFinalityVerifierV1} from "../src/PFTLUniswapPrimaryMarketV2.sol";
import {PFTLUniswapPrimaryMarketV3} from "../src/PFTLUniswapPrimaryMarketV3.sol";
import {PFTLReturnRouteV3} from "../src/PFTLReturnRouteV3.sol";

interface Vm { function prank(address) external; function expectRevert(bytes4) external; function expectRevert(bytes calldata) external; }
contract TokenFixture {
    uint256 public burned;
    function decimals() external pure returns(uint8){return 6;}
    function mint(address,uint256) external {}
    function burnFromBridge(address,uint256 n) external {burned+=n;}
}
contract VerifierFixture {
    function routeTrustClass() external pure returns(bytes32){return keccak256("TRUSTLESS_FINALITY");}
    function isReceiptAccepted(bytes calldata,bytes calldata,bytes calldata,bytes32,bytes32) external pure returns(bool){return false;}
}
contract ReturnGateTest {
    Vm constant vm=Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
    TokenFixture token;
    PFTLUniswapPrimaryMarketV3 market;
    bytes asset=hex"52285e4ded1d58113cfaa12bbc034b8b10213f699b3224c74160450229d6bf7ccfe505c78f2599d975ae2ecf98f7d8af";
    function setUp() public {
        token=new TokenFixture();VerifierFixture verifier=new VerifierFixture();
        PFTLUniswapPrimaryMarketV2.Config memory c=PFTLUniswapPrimaryMarketV2.Config({destinationChainId:block.chainid,
            settlementAssetId:hex"02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b",
            nativeNavAssetId:asset,uniswapPoolId:bytes32(uint256(1)),routeSupplyCapAtoms:2_000_000e6,
            packetNotionalCapAtoms:250_000e6,governance:address(this)});
        market=new PFTLUniswapPrimaryMarketV3(IA666WrappedToken(address(token)),IPFTLReceiptFinalityVerifierV1(address(verifier)),c,
            PFTLReturnRouteV3.ReturnRouteDomain("postfiat-wan-devnet-2",hex"ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9",1,"pftl-b24-ethereum-wB24-usdc-v1"));
    }
    function testReturnDefaultOffAndMintUnpauseCannotEnableBurn() public {
        require(market.returnPaused() && market.mintPaused());
        market.setMintPaused(false);
        vm.expectRevert(PFTLUniswapPrimaryMarketV2.ReturnsPaused.selector);
        market.burnForPftlReturn(1,"pf-owner",asset,bytes32(uint256(1)));
        require(token.burned()==0 && market.totalReturnBurnedAtoms()==0 && !market.consumedReturnNonce(bytes32(uint256(1))));
    }
    function testOnlyGovernanceCanEnableReturns() public {
        vm.prank(address(0x1234));vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.NotGovernance.selector,address(0x1234)));
        market.setReturnPaused(false);require(market.returnPaused());
    }
    function testExplicitFutureEnableKeepsV3BindingAndReplayProtection() public {
        market.setReturnPaused(false);
        bytes32 id=market.burnForPftlReturn(1,"pf-owner",asset,bytes32(uint256(1)));
        require(id!=bytes32(0) && market.returnRouteBinding()!=bytes32(0) && token.burned()==1);
        vm.expectRevert(abi.encodeWithSelector(PFTLUniswapPrimaryMarketV2.ReturnNonceReplay.selector,bytes32(uint256(1))));
        market.burnForPftlReturn(1,"pf-owner",asset,bytes32(uint256(1)));
        market.setReturnPaused(true);vm.expectRevert(PFTLUniswapPrimaryMarketV2.ReturnsPaused.selector);
        market.burnForPftlReturn(1,"pf-owner",asset,bytes32(uint256(2)));require(token.burned()==1);
    }
}
