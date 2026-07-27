// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    A651ToA666MigrationV1,
    IA651SupplyController,
    IERC20MigrationToken
} from "../src/A651ToA666MigrationV1.sol";

interface VmMigration {
    function prank(address sender) external;
    function expectRevert() external;
    function expectRevert(bytes4 selector) external;
}

contract MigrationTokenMock is IERC20MigrationToken {
    uint8 public immutable override decimals;
    mapping(address => uint256) public override balanceOf;

    constructor(uint8 decimals_) {
        decimals = decimals_;
    }

    function mint(address recipient, uint256 amount) external {
        balanceOf[recipient] += amount;
    }

    function burn(address holder, uint256 amount) external {
        balanceOf[holder] -= amount;
    }

    function transfer(address recipient, uint256 amount) external override returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[recipient] += amount;
        return true;
    }
}

contract A651SupplyControllerMock is IA651SupplyController {
    MigrationTokenMock public immutable token;
    address public authorizedMigration;

    constructor(MigrationTokenMock token_) {
        token = token_;
    }

    function authorize(address migration) external {
        authorizedMigration = migration;
    }

    function primaryBurn(address assertedToken, address holder, uint256 amount) external override {
        require(msg.sender == authorizedMigration, "unauthorized migration");
        require(assertedToken == address(token), "wrong token");
        token.burn(holder, amount);
    }
}

contract A651ToA666MigrationV1Test {
    VmMigration private constant vm =
        VmMigration(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant OPENING_A666_ATOMS = 31_386_197_455;
    uint256 private constant SNAPSHOT_A651_ATOMS = 4_000e18;

    MigrationTokenMock private a651;
    MigrationTokenMock private a666;
    A651SupplyControllerMock private controller;
    A651ToA666MigrationV1 private migration;
    address private holder = address(0xB0B);

    function setUp() external {
        a651 = new MigrationTokenMock(18);
        a666 = new MigrationTokenMock(6);
        controller = new A651SupplyControllerMock(a651);
        migration = new A651ToA666MigrationV1(
            controller,
            address(a651),
            a666,
            OPENING_A666_ATOMS,
            SNAPSHOT_A651_ATOMS
        );
        controller.authorize(address(migration));
        a666.mint(address(migration), OPENING_A666_ATOMS);
    }

    function testMigrationBurnsLegacyBeforeReleasingFixedRatioSuccessor() external {
        uint256 oldAmount = 3_744_055735262587944431;
        a651.mint(holder, oldAmount);
        uint256 expected = 29_377_918_147;

        vm.prank(holder);
        uint256 released = migration.migrate(oldAmount, holder);

        require(released == expected, "wrong released amount");
        require(a651.balanceOf(holder) == 0, "legacy balance not burned");
        require(a666.balanceOf(holder) == expected, "wrong successor balance");
        require(migration.totalA651Burned() == oldAmount, "wrong burn total");
        require(migration.totalA666Released() == expected, "wrong release total");
    }

    function testMigrationFailsAtomicallyWhenSuccessorReserveIsInsufficient() external {
        uint256 oldAmount = 4_000e18;
        a651.mint(holder, oldAmount);
        vm.prank(address(migration));
        a666.transfer(address(0xDEAD), 1);

        vm.prank(holder);
        vm.expectRevert();
        migration.migrate(oldAmount, holder);

        require(a651.balanceOf(holder) == oldAmount, "burn was not reverted");
        require(a666.balanceOf(holder) == 0, "successor moved on failure");
    }

    function testDustCannotBeBurnedForZeroSuccessor() external {
        a651.mint(holder, 1);
        vm.prank(holder);
        vm.expectRevert(A651ToA666MigrationV1.ZeroMigrationOutput.selector);
        migration.migrate(1, holder);
        require(a651.balanceOf(holder) == 1, "dust was burned");
    }
}
