// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IOneShotDepositToken {
    function balanceOf(address account) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
}

interface IOneShotPfUsdcVault {
    function depositV2(uint256 amount, string calldata pftlRecipient, bytes32 nonce, bytes32 routeBinding)
        external
        returns (bytes32 depositId);
}

/// @notice Deployment-only helper that deposits USDC pre-funded at its
///         deterministic CREATE address. This lets the deployment account
///         reserve its next nonce exclusively for the immutable verifier.
contract PfUsdcOneShotDepositor {
    error WrongCreateAddress(address observed, address expected);
    error WrongPrefundedBalance(uint256 observed, uint256 expected);
    error ApprovalFailed();
    error ResidualBalance(uint256 observed);

    bytes32 public immutable depositId;

    constructor(
        address expectedCreateAddress,
        IOneShotDepositToken token,
        IOneShotPfUsdcVault vault,
        uint256 amount,
        string memory pftlRecipient,
        bytes32 nonce,
        bytes32 routeBinding
    ) {
        if (address(this) != expectedCreateAddress) {
            revert WrongCreateAddress(address(this), expectedCreateAddress);
        }
        uint256 beforeBalance = token.balanceOf(address(this));
        if (beforeBalance != amount) revert WrongPrefundedBalance(beforeBalance, amount);
        if (!token.approve(address(vault), amount)) revert ApprovalFailed();
        depositId = vault.depositV2(amount, pftlRecipient, nonce, routeBinding);
        uint256 afterBalance = token.balanceOf(address(this));
        if (afterBalance != 0) revert ResidualBalance(afterBalance);
    }
}
