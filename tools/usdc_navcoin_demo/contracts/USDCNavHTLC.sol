// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

interface IERC20 {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 value) external returns (bool);
    function transferFrom(address from, address to, uint256 value) external returns (bool);
}

contract USDCNavHTLC {
    struct Swap {
        address refundAddress;
        address recipient;
        uint128 amount;
        uint64 refundTime;
        bytes32 hashlock;
        uint8 state; // 1=open, 2=redeemed, 3=refunded
    }

    IERC20 public immutable token;
    mapping(bytes32 => Swap) public swaps;

    event Locked(
        bytes32 indexed swapId,
        address indexed refundAddress,
        address indexed recipient,
        uint256 amount,
        bytes32 hashlock,
        uint64 refundTime
    );
    event Redeemed(bytes32 indexed swapId, bytes32 preimage);
    event Refunded(bytes32 indexed swapId);

    constructor(address tokenAddress) {
        require(tokenAddress != address(0), "ZERO_TOKEN");
        require(tokenAddress.code.length != 0, "TOKEN_NOT_CONTRACT");
        token = IERC20(tokenAddress);
    }

    function _safeTransfer(address to, uint256 amount) private {
        (bool success, bytes memory result) =
            address(token).call(abi.encodeCall(IERC20.transfer, (to, amount)));
        require(success && (result.length == 0 || abi.decode(result, (bool))), "TRANSFER_OUT");
    }

    function _safeTransferFrom(address from, address to, uint256 amount) private {
        (bool success, bytes memory result) =
            address(token).call(abi.encodeCall(IERC20.transferFrom, (from, to, amount)));
        require(success && (result.length == 0 || abi.decode(result, (bool))), "TRANSFER_IN");
    }

    function lock(
        bytes32 swapId,
        address recipient,
        uint128 amount,
        bytes32 hashlock,
        uint64 refundTime
    ) external {
        require(swaps[swapId].state == 0, "DUPLICATE_ID");
        require(recipient != address(0), "ZERO_RECIPIENT");
        require(amount != 0, "ZERO_AMOUNT");
        require(refundTime > block.timestamp, "BAD_TIMEOUT");
        swaps[swapId] = Swap({
            refundAddress: msg.sender,
            recipient: recipient,
            amount: amount,
            refundTime: refundTime,
            hashlock: hashlock,
            state: 1
        });
        uint256 beforeBalance = token.balanceOf(address(this));
        _safeTransferFrom(msg.sender, address(this), amount);
        require(token.balanceOf(address(this)) == beforeBalance + amount, "INEXACT_TRANSFER_IN");
        emit Locked(swapId, msg.sender, recipient, amount, hashlock, refundTime);
    }

    function redeem(bytes32 swapId, bytes32 preimage) external {
        Swap storage swap = swaps[swapId];
        require(swap.state == 1, "NOT_OPEN");
        require(block.timestamp < swap.refundTime, "EXPIRED");
        require(sha256(abi.encodePacked(preimage)) == swap.hashlock, "WRONG_PREIMAGE");
        swap.state = 2;
        _safeTransfer(swap.recipient, swap.amount);
        emit Redeemed(swapId, preimage);
    }

    function refund(bytes32 swapId) external {
        Swap storage swap = swaps[swapId];
        require(swap.state == 1, "NOT_OPEN");
        require(block.timestamp >= swap.refundTime, "TOO_EARLY");
        swap.state = 3;
        _safeTransfer(swap.refundAddress, swap.amount);
        emit Refunded(swapId);
    }
}
