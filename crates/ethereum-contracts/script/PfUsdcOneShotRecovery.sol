// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IERC20PfUsdcRecovery {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
}

/// @notice Recovers an exact prefunded CREATE address after a fail-closed
///         deployment attempt invalidated the intended one-shot depositor.
/// @dev Constructor-only and non-reusable. The expected self address prevents
///      deploying this bytecode at any address other than the funded CREATE
///      address, and exact before/after checks reject partial token behavior.
contract PfUsdcOneShotRecovery {
    error WrongCreateAddress(address actual, address expected);
    error UnexpectedBalance(uint256 actual, uint256 expected);
    error TransferFailed();

    constructor(address expectedSelf, IERC20PfUsdcRecovery token, address recipient, uint256 amount) {
        if (address(this) != expectedSelf) revert WrongCreateAddress(address(this), expectedSelf);
        uint256 beforeBalance = token.balanceOf(address(this));
        if (beforeBalance != amount) revert UnexpectedBalance(beforeBalance, amount);
        if (!token.transfer(recipient, amount)) revert TransferFailed();
        uint256 afterBalance = token.balanceOf(address(this));
        if (afterBalance != 0) revert UnexpectedBalance(afterBalance, 0);
    }
}
