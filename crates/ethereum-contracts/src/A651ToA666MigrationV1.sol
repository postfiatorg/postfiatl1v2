// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IA651SupplyController {
    function primaryBurn(address token, address holder, uint256 amount) external;
}

interface IERC20MigrationToken {
    function decimals() external view returns (uint8);
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
}

/// @notice Ownerless, fixed-ratio retirement path from legacy a651 to wA666.
/// @dev The contract must be authorized once as an a651 primary controller and
///      funded only with wA666 whose PFTL issuance represents the frozen a651
///      holder snapshot. It cannot mint either token or change the conversion.
contract A651ToA666MigrationV1 {
    error ZeroAddress(bytes32 field);
    error ZeroValue(bytes32 field);
    error WrongDecimals(uint8 a651Decimals, uint8 a666Decimals);
    error ZeroMigrationOutput();
    error InsufficientA666Reserve(uint256 available, uint256 required);
    error TransferFailed();
    error ReentrantCall();

    event Migrated(
        address indexed holder,
        address indexed recipient,
        uint256 a651Burned,
        uint256 a666Released
    );

    IA651SupplyController public immutable a651SupplyController;
    address public immutable a651Token;
    IERC20MigrationToken public immutable a666Token;
    uint256 public immutable a666NumeratorAtoms;
    uint256 public immutable a651DenominatorAtoms;

    uint256 public totalA651Burned;
    uint256 public totalA666Released;
    uint256 private reentrancyLock;

    modifier nonReentrant() {
        if (reentrancyLock != 0) revert ReentrantCall();
        reentrancyLock = 1;
        _;
        reentrancyLock = 0;
    }

    constructor(
        IA651SupplyController a651SupplyController_,
        address a651Token_,
        IERC20MigrationToken a666Token_,
        uint256 a666NumeratorAtoms_,
        uint256 a651DenominatorAtoms_
    ) {
        if (address(a651SupplyController_) == address(0)) {
            revert ZeroAddress("a651_supply_controller");
        }
        if (a651Token_ == address(0)) revert ZeroAddress("a651_token");
        if (address(a666Token_) == address(0)) revert ZeroAddress("a666_token");
        if (a666NumeratorAtoms_ == 0 || a651DenominatorAtoms_ == 0) {
            revert ZeroValue("conversion");
        }
        uint8 oldDecimals = IERC20MigrationToken(a651Token_).decimals();
        uint8 newDecimals = a666Token_.decimals();
        if (oldDecimals != 18 || newDecimals != 6) {
            revert WrongDecimals(oldDecimals, newDecimals);
        }

        a651SupplyController = a651SupplyController_;
        a651Token = a651Token_;
        a666Token = a666Token_;
        a666NumeratorAtoms = a666NumeratorAtoms_;
        a651DenominatorAtoms = a651DenominatorAtoms_;
    }

    function quote(uint256 a651Amount) public view returns (uint256) {
        return (a651Amount * a666NumeratorAtoms) / a651DenominatorAtoms;
    }

    function remainingA666Reserve() external view returns (uint256) {
        return a666Token.balanceOf(address(this));
    }

    function migrate(uint256 a651Amount, address recipient)
        external
        nonReentrant
        returns (uint256 a666Amount)
    {
        if (a651Amount == 0) revert ZeroValue("a651_amount");
        if (recipient == address(0)) revert ZeroAddress("recipient");
        a666Amount = quote(a651Amount);
        if (a666Amount == 0) revert ZeroMigrationOutput();
        uint256 available = a666Token.balanceOf(address(this));
        if (available < a666Amount) {
            revert InsufficientA666Reserve(available, a666Amount);
        }

        // The legacy controller burns directly from the caller. If this call
        // fails, no successor inventory moves.
        a651SupplyController.primaryBurn(a651Token, msg.sender, a651Amount);
        totalA651Burned += a651Amount;
        totalA666Released += a666Amount;
        if (!a666Token.transfer(recipient, a666Amount)) revert TransferFailed();
        emit Migrated(msg.sender, recipient, a651Amount, a666Amount);
    }
}
