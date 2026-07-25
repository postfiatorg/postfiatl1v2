// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

import "../contracts/MockUSDC.sol";
import "../contracts/USDCNavHTLC.sol";

interface Vm {
    function warp(uint256) external;
    function prank(address) external;
    function expectRevert(bytes calldata) external;
}

contract USDCNavHTLCTest {
    Vm private constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
    address private constant USER = address(0x1001);
    address private constant COORDINATOR = address(0x2002);
    bytes32 private constant PREIMAGE = bytes32(uint256(0xA11CE));
    bytes32 private constant HASHLOCK =
        0x16f62e786d3b8845fb7a54b53a7ebcb64c6c86ab05e75577302e4f152d857847;

    MockUSDC private usdc;
    USDCNavHTLC private htlc;

    function setUp() public {
        usdc = new MockUSDC(USER, COORDINATOR, 1_000_000);
        htlc = new USDCNavHTLC(address(usdc));
        vm.prank(USER);
        usdc.approve(address(htlc), type(uint256).max);
    }

    function testSha256RedeemAndDuplicateFail() public {
        require(sha256(abi.encodePacked(PREIMAGE)) == HASHLOCK, "fixture");
        vm.prank(USER);
        htlc.lock(bytes32("happy"), COORDINATOR, 100_000, HASHLOCK, uint64(block.timestamp + 60));
        htlc.redeem(bytes32("happy"), PREIMAGE);
        require(usdc.balanceOf(COORDINATOR) == 1_100_000, "recipient balance");
        vm.expectRevert(bytes("NOT_OPEN"));
        htlc.redeem(bytes32("happy"), PREIMAGE);
    }

    function testWrongPreimageEarlyRefundAndLateRedeemFail() public {
        vm.prank(USER);
        htlc.lock(bytes32("refund"), COORDINATOR, 25_000, HASHLOCK, uint64(block.timestamp + 60));
        vm.expectRevert(bytes("WRONG_PREIMAGE"));
        htlc.redeem(bytes32("refund"), bytes32(uint256(7)));
        vm.prank(USER);
        vm.expectRevert(bytes("TOO_EARLY"));
        htlc.refund(bytes32("refund"));
        vm.warp(block.timestamp + 60);
        vm.expectRevert(bytes("EXPIRED"));
        htlc.redeem(bytes32("refund"), PREIMAGE);
        vm.prank(USER);
        htlc.refund(bytes32("refund"));
        require(usdc.balanceOf(USER) == 1_000_000, "refund conservation");
    }
}
