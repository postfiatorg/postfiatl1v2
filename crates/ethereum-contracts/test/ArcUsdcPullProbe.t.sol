// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ArcUsdcPullProbe} from "../src/ArcUsdcPullProbe.sol";

contract MockArcUsdc {
    string public constant name = "USDC";
    string public constant symbol = "USDC";
    uint8 public constant decimals = 6;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    function mint(address recipient, uint256 amount) external {
        balanceOf[recipient] += amount;
        emit Transfer(address(0), recipient, amount);
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 available = allowance[from][msg.sender];
        require(available >= amount, "allowance");
        require(balanceOf[from] >= amount, "balance");
        allowance[from][msg.sender] = available - amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        emit Transfer(from, to, amount);
        return true;
    }
}

contract ArcUsdcPullProbeTest {
    ArcUsdcPullProbe private probe;
    MockArcUsdc private token;

    function setUp() public {
        probe = new ArcUsdcPullProbe();
        token = new MockArcUsdc();
        token.mint(address(this), 1_000_000);
    }

    function testApproveAndVaultStylePullOneUsdc() public {
        require(token.approve(address(probe), 1_000_000), "approve failed");
        probe.pull(address(token), 1_000_000);

        require(token.balanceOf(address(probe)) == 1_000_000, "probe balance mismatch");
        require(token.balanceOf(address(this)) == 0, "sender balance mismatch");
        require(token.allowance(address(this), address(probe)) == 0, "allowance not consumed");
    }

    function testPullRejectsWithoutAllowance() public {
        (bool ok,) = address(probe).call(abi.encodeCall(ArcUsdcPullProbe.pull, (address(token), 1_000_000)));
        require(!ok, "unapproved pull unexpectedly succeeded");
    }
}
