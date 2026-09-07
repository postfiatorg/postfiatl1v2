// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IArcUsdc {
    function decimals() external view returns (uint8);
    function balanceOf(address account) external view returns (uint256);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

/// @notice Testnet-only probe for the ERC-20 assumptions used by ERC20BridgeVaultV2.
contract ArcUsdcPullProbe {
    error WrongDecimals(uint8 actual);
    error TransferFromFailed();
    error BalanceDeltaMismatch(uint256 expected, uint256 actual);

    event Pulled(address indexed token, address indexed from, uint256 amount);

    function pull(address token, uint256 amount) external {
        uint8 tokenDecimals = IArcUsdc(token).decimals();
        if (tokenDecimals != 6) revert WrongDecimals(tokenDecimals);

        uint256 beforeBalance = IArcUsdc(token).balanceOf(address(this));
        if (!IArcUsdc(token).transferFrom(msg.sender, address(this), amount)) {
            revert TransferFromFailed();
        }
        uint256 received = IArcUsdc(token).balanceOf(address(this)) - beforeBalance;
        if (received != amount) revert BalanceDeltaMismatch(amount, received);

        emit Pulled(token, msg.sender, amount);
    }
}
