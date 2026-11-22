// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;
import {PFTLReceiptFinalityVerifierV2,IPFTLReceiptSP1VerifierV2} from "../src/PFTLReceiptFinalityVerifierV2.sol";
import {WrappedVenueNAVCoin} from "../src/PFTLUniswapHandoffController.sol";
import {PFTLUniswapPrimaryMarketV2,IA666WrappedToken,IPFTLReceiptFinalityVerifierV1} from "../src/PFTLUniswapPrimaryMarketV2.sol";
import {PFTLUniswapPrimaryMarketV3} from "../src/PFTLUniswapPrimaryMarketV3.sol";
import {PFTLReturnRouteV3} from "../src/PFTLReturnRouteV3.sol";
interface DeployVm {
    function chainId(uint256) external; function startPrank(address) external; function stopPrank() external;
    function etch(address,bytes calldata) external; function parseBytes(string calldata) external pure returns(bytes memory);
    function readFile(string calldata) external view returns(string memory);
    function serializeAddress(string calldata,string calldata,address) external returns(string memory);
    function serializeBool(string calldata,string calldata,bool) external returns(string memory);
    function serializeBytes32(string calldata,string calldata,bytes32) external returns(string memory);
    function writeJson(string calldata,string calldata) external;
}
/// Constructor/binding test only. Checkpoint pins and deployer are explicit fixtures.
contract DeploymentPinsTest {
    DeployVm constant vm=DeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    function testPredictedFreshDeploymentAndReturnGate() public {
        vm.chainId(1);
        address deployer=address(uint160(uint256(keccak256("PUBLIC REHEARSAL ONLY B24 deployer 20261114"))));
        address tokenExpected=address(uint160(uint256(keccak256(abi.encodePacked(hex"d694",deployer,hex"80")))));
        address verifierExpected=address(uint160(uint256(keccak256(abi.encodePacked(hex"d694",deployer,hex"01")))));
        address controllerExpected=address(uint160(uint256(keccak256(abi.encodePacked(hex"d694",deployer,hex"02")))));
        address sp1=address(0x397A5f7f3dBd538f23DE225B51f532c34448dA9B);
        vm.etch(sp1,vm.parseBytes(vm.readFile("/home/postfiat/repos/b23-coord/evidence/tee/phase4/sp1-runtime.hex")));
        bytes memory genesis=hex"ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9";
        bytes memory asset=hex"52285e4ded1d58113cfaa12bbc034b8b10213f699b3224c74160450229d6bf7ccfe505c78f2599d975ae2ecf98f7d8af";
        bytes memory settlement=hex"02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";
        string memory route="pftl-b24-ethereum-wB24-usdc-v1";
        vm.startPrank(deployer);
        WrappedVenueNAVCoin token=new WrappedVenueNAVCoin("Wrapped Schwab NAVCoin B24","wB24",6,deployer);
        PFTLReceiptFinalityVerifierV2.Config memory v;
        v.sp1Verifier=IPFTLReceiptSP1VerifierV2(sp1);v.sp1VerifierRuntimeCodeHash=sp1.codehash;v.sp1ProofSelector=0x4388a21c;
        v.programVKey=0x0060d83567657466eaac782e0be174b57277df6ac349f42288852a93d72f0fe1;
        v.pftlChainIdHash=keccak256("postfiat-wan-devnet-2");v.pftlGenesisHashCommitment=keccak256(genesis);v.pftlProtocolVersion=1;
        v.routeIdCommitment=keccak256(bytes(route));v.nativeNavAssetIdCommitment=keccak256(asset);v.settlementAssetIdCommitment=keccak256(settlement);
        v.destinationChainId=1;v.controller=controllerExpected;v.wrappedToken=address(token);v.wrappedTokenRuntimeCodeHash=address(token).codehash;
        v.maxProofBytes=356;v.maxPublicValuesBytes=1120;
        v.initialCheckpointCommitment=keccak256("UNQUALIFIED REHEARSAL CHECKPOINT");
        v.initialCommitteeRootCommitment=keccak256("UNQUALIFIED REHEARSAL COMMITTEE");v.initialFinalizedHeight=1310;
        PFTLReceiptFinalityVerifierV2 verifier=new PFTLReceiptFinalityVerifierV2(v);
        bytes32 poolId=keccak256(abi.encode(tokenExpected,address(0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48),uint24(500),int24(10),address(0)));
        PFTLUniswapPrimaryMarketV2.Config memory c=PFTLUniswapPrimaryMarketV2.Config(1,settlement,asset,poolId,2_000_000e6,250_000e6,deployer);
        PFTLUniswapPrimaryMarketV3 controller=new PFTLUniswapPrimaryMarketV3(IA666WrappedToken(address(token)),IPFTLReceiptFinalityVerifierV1(address(verifier)),c,
            PFTLReturnRouteV3.ReturnRouteDomain("postfiat-wan-devnet-2",genesis,1,route));
        require(address(token)==tokenExpected && address(verifier)==verifierExpected && address(controller)==controllerExpected,"CREATE prediction");
        token.setController(address(controller));token.lockController();
        require(token.controller_locked() && controller.mintPaused() && controller.returnPaused() && token.totalSupply()==0,"initial state");
        vm.stopPrank();
        string memory id="deployment";
        vm.serializeAddress(id,"token",address(token));vm.serializeAddress(id,"verifier",address(verifier));vm.serializeAddress(id,"controller",address(controller));
        vm.serializeBool(id,"passed",true);vm.serializeBool(id,"fixture_checkpoint_and_roles",true);vm.serializeBool(id,"mainnet_deployed",false);
        vm.serializeBool(id,"return_paused",controller.returnPaused());vm.serializeBool(id,"mint_paused",controller.mintPaused());
        vm.serializeBytes32(id,"token_runtime_code_hash",address(token).codehash);vm.serializeBytes32(id,"verifier_runtime_code_hash",address(verifier).codehash);
        string memory result=vm.serializeBytes32(id,"controller_runtime_code_hash",address(controller).codehash);
        vm.writeJson(result,"/home/postfiat/repos/b23-coord/evidence/tee/phase4/deployment-pins.json");
    }
}
